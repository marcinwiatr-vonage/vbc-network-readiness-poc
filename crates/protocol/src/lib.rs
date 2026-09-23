//! Versioned binary wire protocol for authenticated network-readiness frames.

use hmac::{Hmac, Mac};
use sha2::Sha256;

/// The four-byte protocol discriminator present at the start of every frame.
pub const MAGIC: [u8; 4] = *b"NRP1";
/// First supported version of the network-readiness packet format.
pub const PROTOCOL_VERSION: u8 = 1;
/// Largest permitted application payload in one UDP frame.
pub const MAX_PAYLOAD_LENGTH: usize = 1_200;

const HEADER_LENGTH: usize = 40;
const AUTH_TAG_LENGTH: usize = 32;

type HmacSha256 = Hmac<Sha256>;

/// Direction relative to the endpoint agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Direction {
    /// Packet sent from the endpoint agent to the regional probe.
    Uplink = 1,
    /// Packet sent from the regional probe to the endpoint agent.
    Downlink = 2,
}

impl TryFrom<u8> for Direction {
    type Error = PacketError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Uplink),
            2 => Ok(Self::Downlink),
            _ => Err(PacketError::UnknownDirection(value)),
        }
    }
}

/// An authenticated UDP frame exchanged by the endpoint agent and probe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    test_id: [u8; 16],
    direction: Direction,
    sequence_number: u64,
    monotonic_timestamp_ns: u64,
    payload: Vec<u8>,
}

impl Packet {
    /// Creates a frame. Callers should use [`Self::try_new`] when accepting an
    /// externally supplied payload.
    pub fn new(
        test_id: [u8; 16],
        direction: Direction,
        sequence_number: u64,
        monotonic_timestamp_ns: u64,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            test_id,
            direction,
            sequence_number,
            monotonic_timestamp_ns,
            payload,
        }
    }

    /// Creates a frame while enforcing the protocol payload ceiling.
    pub fn try_new(
        test_id: [u8; 16],
        direction: Direction,
        sequence_number: u64,
        monotonic_timestamp_ns: u64,
        payload: Vec<u8>,
    ) -> Result<Self, PacketError> {
        if payload.len() > MAX_PAYLOAD_LENGTH {
            return Err(PacketError::PayloadTooLarge(payload.len()));
        }

        Ok(Self::new(
            test_id,
            direction,
            sequence_number,
            monotonic_timestamp_ns,
            payload,
        ))
    }

    /// Returns the opaque session identifier carried by this frame.
    pub const fn test_id(&self) -> [u8; 16] {
        self.test_id
    }

    /// Returns the direction relative to the endpoint agent.
    pub const fn direction(&self) -> Direction {
        self.direction
    }

    /// Returns the wire-format version carried by this packet.
    pub const fn protocol_version(&self) -> u8 {
        PROTOCOL_VERSION
    }

    /// Serializes and authenticates this packet with HMAC-SHA-256.
    pub fn encode(&self, authentication_key: &[u8]) -> Vec<u8> {
        assert!(
            self.payload.len() <= MAX_PAYLOAD_LENGTH,
            "Packet::new payload exceeds MAX_PAYLOAD_LENGTH; use Packet::try_new for untrusted input"
        );

        let mut bytes = Vec::with_capacity(HEADER_LENGTH + self.payload.len() + AUTH_TAG_LENGTH);
        bytes.extend_from_slice(&MAGIC);
        bytes.push(PROTOCOL_VERSION);
        bytes.push(self.direction as u8);
        bytes.extend_from_slice(&self.test_id);
        bytes.extend_from_slice(&self.sequence_number.to_be_bytes());
        bytes.extend_from_slice(&self.monotonic_timestamp_ns.to_be_bytes());
        bytes.extend_from_slice(&(self.payload.len() as u16).to_be_bytes());
        bytes.extend_from_slice(&self.payload);

        let tag = authentication_tag(authentication_key, &bytes);
        bytes.extend_from_slice(&tag);
        bytes
    }

    /// Verifies and decodes one complete wire frame.
    pub fn decode(bytes: &[u8], authentication_key: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() < HEADER_LENGTH + AUTH_TAG_LENGTH {
            return Err(PacketError::Truncated);
        }

        if bytes[..4] != MAGIC {
            return Err(PacketError::InvalidMagic);
        }

        let version = bytes[4];
        if version != PROTOCOL_VERSION {
            return Err(PacketError::UnsupportedVersion(version));
        }

        let direction = Direction::try_from(bytes[5])?;
        let mut test_id = [0_u8; 16];
        test_id.copy_from_slice(&bytes[6..22]);
        let sequence_number =
            u64::from_be_bytes(bytes[22..30].try_into().expect("fixed header slice"));
        let monotonic_timestamp_ns =
            u64::from_be_bytes(bytes[30..38].try_into().expect("fixed header slice"));
        let payload_length =
            u16::from_be_bytes(bytes[38..40].try_into().expect("fixed header slice")) as usize;

        if payload_length > MAX_PAYLOAD_LENGTH {
            return Err(PacketError::PayloadTooLarge(payload_length));
        }

        let expected_length = HEADER_LENGTH + payload_length + AUTH_TAG_LENGTH;
        if bytes.len() != expected_length {
            return Err(PacketError::LengthMismatch {
                expected: expected_length,
                actual: bytes.len(),
            });
        }

        let payload_end = HEADER_LENGTH + payload_length;
        let expected_tag = &bytes[payload_end..];
        let mut mac = HmacSha256::new_from_slice(authentication_key)
            .expect("HMAC-SHA-256 accepts authentication keys of any length");
        mac.update(&bytes[..payload_end]);
        mac.verify_slice(expected_tag)
            .map_err(|_| PacketError::AuthenticationFailed)?;

        Self::try_new(
            test_id,
            direction,
            sequence_number,
            monotonic_timestamp_ns,
            bytes[HEADER_LENGTH..payload_end].to_vec(),
        )
    }
}

/// Errors returned while constructing or validating a packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PacketError {
    /// Datagram is shorter than the smallest valid authenticated frame.
    Truncated,
    /// The leading protocol discriminator differs from [`MAGIC`].
    InvalidMagic,
    /// The frame advertises a protocol version this binary does not support.
    UnsupportedVersion(u8),
    /// The frame uses an unknown direction value.
    UnknownDirection(u8),
    /// Payload length exceeds [`MAX_PAYLOAD_LENGTH`].
    PayloadTooLarge(usize),
    /// Datagram length disagrees with the fixed header's payload length.
    LengthMismatch { expected: usize, actual: usize },
    /// HMAC-SHA-256 verification failed.
    AuthenticationFailed,
}

impl core::fmt::Display for PacketError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("packet is truncated"),
            Self::InvalidMagic => formatter.write_str("packet has an invalid magic value"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported packet protocol version {version}")
            }
            Self::UnknownDirection(direction) => {
                write!(formatter, "unknown packet direction {direction}")
            }
            Self::PayloadTooLarge(length) => {
                write!(
                    formatter,
                    "packet payload length {length} exceeds the configured maximum"
                )
            }
            Self::LengthMismatch { expected, actual } => {
                write!(
                    formatter,
                    "packet length mismatch: expected {expected}, received {actual}"
                )
            }
            Self::AuthenticationFailed => formatter.write_str("packet authentication failed"),
        }
    }
}

impl std::error::Error for PacketError {}

fn authentication_tag(
    authentication_key: &[u8],
    authenticated_bytes: &[u8],
) -> [u8; AUTH_TAG_LENGTH] {
    let mut mac = HmacSha256::new_from_slice(authentication_key)
        .expect("HMAC-SHA-256 accepts authentication keys of any length");
    mac.update(authenticated_bytes);
    mac.finalize().into_bytes().into()
}
