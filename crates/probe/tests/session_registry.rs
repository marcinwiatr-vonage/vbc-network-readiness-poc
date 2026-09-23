use probe::{LocalSession, SessionAuthorization, SessionRegistry, serve_one};
use protocol::{Direction, Packet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::thread;
use std::time::{Duration, SystemTime};

#[test]
fn unknown_session_is_not_authorized() {
    let registry = SessionRegistry;
    let source = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49152);

    let authorization = registry.authorize(
        [0x11; 16],
        source,
        SystemTime::now() + Duration::from_secs(1),
    );

    assert_eq!(authorization, SessionAuthorization::Unknown);
}

#[test]
fn probe_rejects_a_non_loopback_bind_address() {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("bind wildcard test socket");
    let session = LocalSession::new(
        [0x11; 16],
        [0x22; 32],
        SystemTime::now() + Duration::from_secs(2),
    );

    assert!(!serve_one(socket, session));
}

#[test]
fn downlink_direction_sent_to_probe_receives_no_udp_response() {
    let session_id = [0x11; 16];
    let key = [0x22; 32];
    let probe_socket = UdpSocket::bind("127.0.0.1:0").expect("bind loopback probe");
    let probe_address = probe_socket.local_addr().expect("read probe address");
    let session = LocalSession::new(session_id, key, SystemTime::now() + Duration::from_secs(2));
    let probe_thread = thread::spawn(move || serve_one(probe_socket, session));

    let agent_socket = UdpSocket::bind("127.0.0.1:0").expect("bind loopback client");
    agent_socket
        .set_read_timeout(Some(Duration::from_millis(150)))
        .expect("set receive timeout");
    let wrong_direction = Packet::new(session_id, Direction::Downlink, 0, 0, vec![0_u8; 172]);
    agent_socket
        .send_to(&wrong_direction.encode(&key), probe_address)
        .expect("send wrong-direction packet");

    let mut buffer = [0_u8; 1_272];
    assert!(
        agent_socket.recv_from(&mut buffer).is_err(),
        "probe must remain silent"
    );
    assert!(!probe_thread.join().expect("probe thread must not panic"));
}

#[test]
fn unknown_test_id_receives_no_udp_response() {
    let session_id = [0x11; 16];
    let key = [0x22; 32];
    let probe_socket = UdpSocket::bind("127.0.0.1:0").expect("bind loopback probe");
    let probe_address = probe_socket.local_addr().expect("read probe address");
    let session = LocalSession::new(session_id, key, SystemTime::now() + Duration::from_secs(2));
    let probe_thread = thread::spawn(move || serve_one(probe_socket, session));

    let agent_socket = UdpSocket::bind("127.0.0.1:0").expect("bind loopback client");
    agent_socket
        .set_read_timeout(Some(Duration::from_millis(150)))
        .expect("set receive timeout");
    let unknown = Packet::new([0x33; 16], Direction::Uplink, 0, 0, vec![0_u8; 172]);
    agent_socket
        .send_to(&unknown.encode(&key), probe_address)
        .expect("send authenticated unknown-session packet");

    let mut buffer = [0_u8; 1_272];
    let error = agent_socket
        .recv_from(&mut buffer)
        .expect_err("unknown session must not receive a response");

    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
    assert!(!probe_thread.join().expect("probe thread must not panic"));
}
