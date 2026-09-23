use protocol::{Direction, PROTOCOL_VERSION, Packet};

#[test]
fn packet_round_trips_through_binary_encoding() {
    let packet = Packet::new(
        [0x11; 16],
        Direction::Uplink,
        42,
        123_456_789,
        vec![0xAB; 172],
    );

    let authentication_key = [0x22; 32];
    let encoded = packet.encode(&authentication_key);
    let decoded = Packet::decode(&encoded, &authentication_key)
        .expect("valid authenticated packet must decode");

    assert_eq!(decoded.protocol_version(), PROTOCOL_VERSION);
    assert_eq!(decoded.test_id(), [0x11; 16]);
    assert_eq!(decoded.direction(), Direction::Uplink);
    assert_eq!(decoded, packet);
}
