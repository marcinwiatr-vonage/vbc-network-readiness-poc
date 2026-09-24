//! Versioned binary wire protocol for authenticated network-readiness frames.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The four-byte protocol discriminator present at the start of every frame.
pub const MAGIC: [u8; 4] = [0x4E, 0x52, 0x50, 0x02];
/// Second version of the wire format; the experimental v1 layout is retired.
pub const PROTOCOL_VERSION: u8 = 2;
/// Largest permitted application payload in one UDP frame.
pub const MAX_PAYLOAD_LENGTH: usize = 1_200;
/// Maximum accepted size of a local bootstrap session file.
pub const LOCAL_SESSION_FILE_MAX_BYTES: usize = 1_024;

const HEADER_LENGTH: usize = 36;
const AUTH_TAG_LENGTH: usize = 32;

type HmacSha256 = Hmac<Sha256>;

/// Strict, local-only bootstrap data shared by the probe and agent CLIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalSessionFile {
    test_id: [u8; 16],
    hmac_key: [u8; 32],
    expires_at_unix_seconds: u64,
    probe_address: SocketAddr,
}

impl LocalSessionFile {
    /// Parses the canonical five-line `NRP-LOCAL-SESSION-V1` contract.
    pub fn parse(bytes: &[u8]) -> Result<Self, LocalSessionFileError> {
        if bytes.len() > LOCAL_SESSION_FILE_MAX_BYTES {
            return Err(LocalSessionFileError::TooLarge);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| LocalSessionFileError::InvalidUtf8)?;
        let text = text
            .strip_suffix('\n')
            .ok_or(LocalSessionFileError::InvalidFormat(
                "missing final newline",
            ))?;
        let lines: Vec<_> = text.split('\n').collect();
        if lines.len() != 5 || lines[0] != "NRP-LOCAL-SESSION-V1" {
            return Err(LocalSessionFileError::InvalidFormat(
                "expected the canonical five-line v1 session format",
            ));
        }

        let test_id = parse_hex_field::<16>(lines[1], "test_id_hex=", "test_id_hex")?;
        let hmac_key = parse_hex_field::<32>(lines[2], "hmac_key_hex=", "hmac_key_hex")?;
        let expires_at = lines[3].strip_prefix("expires_at_unix_seconds=").ok_or(
            LocalSessionFileError::InvalidFormat("expected expires_at_unix_seconds as line four"),
        )?;
        if expires_at.is_empty()
            || !expires_at.bytes().all(|byte| byte.is_ascii_digit())
            || (expires_at.len() > 1 && expires_at.starts_with('0'))
        {
            return Err(LocalSessionFileError::InvalidExpiry);
        }
        let expires_at_unix_seconds = expires_at
            .parse()
            .map_err(|_| LocalSessionFileError::InvalidExpiry)?;
        let probe_address_text =
            lines[4]
                .strip_prefix("probe_address=")
                .ok_or(LocalSessionFileError::InvalidFormat(
                    "expected probe_address as line five",
                ))?;
        if !matches!(probe_address_text, "127.0.0.1:10000" | "127.0.0.1:16384") {
            return Err(LocalSessionFileError::InvalidAddress);
        }
        let probe_address: SocketAddr = probe_address_text
            .parse()
            .map_err(|_| LocalSessionFileError::InvalidAddress)?;
        if probe_address.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST)
            || !matches!(probe_address.port(), 10_000 | 16_384)
        {
            return Err(LocalSessionFileError::InvalidAddress);
        }

        Ok(Self {
            test_id,
            hmac_key,
            expires_at_unix_seconds,
            probe_address,
        })
    }

    pub const fn test_id(self) -> [u8; 16] {
        self.test_id
    }

    pub const fn hmac_key(self) -> [u8; 32] {
        self.hmac_key
    }

    pub const fn expires_at_unix_seconds(self) -> u64 {
        self.expires_at_unix_seconds
    }

    pub const fn probe_address(self) -> SocketAddr {
        self.probe_address
    }

    pub fn expires_at(self) -> Result<SystemTime, LocalSessionFileError> {
        UNIX_EPOCH
            .checked_add(Duration::from_secs(self.expires_at_unix_seconds))
            .ok_or(LocalSessionFileError::InvalidExpiry)
    }
}

fn parse_hex_field<const N: usize>(
    line: &str,
    prefix: &'static str,
    field: &'static str,
) -> Result<[u8; N], LocalSessionFileError> {
    let encoded = line
        .strip_prefix(prefix)
        .ok_or(LocalSessionFileError::InvalidFormat(field))?;
    if encoded.len() != N * 2
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(LocalSessionFileError::InvalidHex(field));
    }
    let mut decoded = [0_u8; N];
    let (pairs, remainder) = encoded.as_bytes().as_chunks::<2>();
    debug_assert!(remainder.is_empty());
    for (index, pair) in pairs.iter().enumerate() {
        decoded[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(decoded)
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("hex was validated before decoding"),
    }
}

/// Validation failures for the local bootstrap session-file contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalSessionFileError {
    TooLarge,
    InvalidUtf8,
    InvalidFormat(&'static str),
    InvalidHex(&'static str),
    InvalidExpiry,
    InvalidAddress,
}

impl core::fmt::Display for LocalSessionFileError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("local session file exceeds 1024 bytes"),
            Self::InvalidUtf8 => formatter.write_str("local session file is not valid UTF-8"),
            Self::InvalidFormat(detail) => {
                write!(formatter, "invalid local session format: {detail}")
            }
            Self::InvalidHex(field) => {
                write!(formatter, "invalid canonical lowercase hex in {field}")
            }
            Self::InvalidExpiry => formatter.write_str("invalid local session expiry"),
            Self::InvalidAddress => {
                formatter.write_str("probe_address must be 127.0.0.1 on UDP port 10000 or 16384")
            }
        }
    }
}

impl std::error::Error for LocalSessionFileError {}

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
    sequence_number: u32,
    monotonic_timestamp_ns: i64,
    payload: Vec<u8>,
}

impl Packet {
    /// Creates a frame. Callers should use [`Self::try_new`] when accepting an
    /// externally supplied payload.
    pub fn new(
        test_id: [u8; 16],
        direction: Direction,
        sequence_number: u32,
        monotonic_timestamp_ns: i64,
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
        sequence_number: u32,
        monotonic_timestamp_ns: i64,
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

    /// Returns the per-direction sequence number used for replay admission.
    pub const fn sequence_number(&self) -> u32 {
        self.sequence_number
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
        bytes.extend_from_slice(&self.test_id);
        bytes.push(self.direction as u8);
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

        let mut test_id = [0_u8; 16];
        test_id.copy_from_slice(&bytes[5..21]);
        let direction = Direction::try_from(bytes[21])?;
        let sequence_number =
            u32::from_be_bytes(bytes[22..26].try_into().expect("fixed header slice"));
        let monotonic_timestamp_ns =
            i64::from_be_bytes(bytes[26..34].try_into().expect("fixed header slice"));
        let payload_length =
            u16::from_be_bytes(bytes[34..36].try_into().expect("fixed header slice")) as usize;

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
