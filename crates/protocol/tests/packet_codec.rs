use protocol::{Direction, PROTOCOL_VERSION, Packet, PacketError};

fn encoded_packet() -> (Vec<u8>, [u8; 32]) {
    let key = [0x22; 32];
    let packet = Packet::new(
        [0x11; 16],
        Direction::Uplink,
        42,
        -123_456_789,
        vec![0xAB; 172],
    );
    (packet.encode(&key), key)
}

#[test]
fn packet_round_trips_through_v2_binary_encoding() {
    let (encoded, key) = encoded_packet();
    assert_eq!(PROTOCOL_VERSION, 2);
    assert_eq!(&encoded[..4], &[0x4E, 0x52, 0x50, 0x02]);
    assert_eq!(encoded[4], 2);
    assert_eq!(&encoded[5..21], &[0x11; 16]);
    assert_eq!(encoded[21], Direction::Uplink as u8);
    assert_eq!(&encoded[22..26], &42_u32.to_be_bytes());
    assert_eq!(&encoded[26..34], &(-123_456_789_i64).to_be_bytes());
    assert_eq!(&encoded[34..36], &172_u16.to_be_bytes());
    assert_eq!(encoded[36], 0xAB);
    assert_eq!(encoded.len(), 68 + 172);
    let decoded = Packet::decode(&encoded, &key).expect("valid v2 packet");
    assert_eq!(decoded.sequence_number(), 42);
}

#[test]
fn retired_v1_magic_is_rejected() {
    let (mut encoded, key) = encoded_packet();
    encoded[..4].copy_from_slice(b"NRP1");
    assert_eq!(
        Packet::decode(&encoded, &key),
        Err(PacketError::InvalidMagic)
    );
}

#[test]
fn authentication_and_framing_failures_are_rejected() {
    let (encoded, key) = encoded_packet();
    let wrong_key = [0x33; 32];
    assert_eq!(
        Packet::decode(&encoded, &wrong_key),
        Err(PacketError::AuthenticationFailed)
    );

    let mut tampered = encoded.clone();
    tampered[36] ^= 1;
    assert_eq!(
        Packet::decode(&tampered, &key),
        Err(PacketError::AuthenticationFailed)
    );

    let mut bad_direction = encoded.clone();
    bad_direction[21] = 3;
    assert_eq!(
        Packet::decode(&bad_direction, &key),
        Err(PacketError::UnknownDirection(3))
    );

    let mut bad_version = encoded.clone();
    bad_version[4] = 3;
    assert_eq!(
        Packet::decode(&bad_version, &key),
        Err(PacketError::UnsupportedVersion(3))
    );

    let mut bad_length = encoded.clone();
    bad_length[34..36].copy_from_slice(&173_u16.to_be_bytes());
    assert!(matches!(
        Packet::decode(&bad_length, &key),
        Err(PacketError::LengthMismatch { .. })
    ));

    assert_eq!(
        Packet::decode(&encoded[..67], &key),
        Err(PacketError::Truncated)
    );
}
