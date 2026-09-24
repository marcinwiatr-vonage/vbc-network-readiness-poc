use probe::{
    LocalSession, MAX_SESSION_DURATION, MAX_SESSION_PACKETS, ProbeRun, SessionAuthorization,
    SessionRegistry, serve_session,
};
use protocol::{Direction, Packet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

fn receive_downlink(socket: &UdpSocket, key: &[u8; 32], expected_sequence: u32) {
    let mut buffer = [0_u8; 1_272];
    let (received, _) = socket
        .recv_from(&mut buffer)
        .expect("receive authenticated downlink");
    let packet = Packet::decode(&buffer[..received], key).expect("decode downlink");
    assert_eq!(packet.direction(), Direction::Downlink);
    assert_eq!(packet.sequence_number(), expected_sequence);
}

#[test]
fn registry_rejects_at_monotonic_expiry_boundary() {
    let test_id = [0x44; 16];
    let source = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49154);
    let deadline = std::time::Instant::now();
    let registry = SessionRegistry::with_session(test_id, deadline);

    assert!(
        registry
            .authorize_uplink(test_id, source, 1, deadline)
            .is_none()
    );
}

#[test]
fn registry_binds_first_source_and_rejects_rebinding() {
    let test_id = [0x11; 16];
    let first_source = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49152);
    let second_source = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49153);
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let registry = SessionRegistry::with_session(test_id, deadline);

    let first_permit = registry
        .authorize_uplink(test_id, first_source, 1, std::time::Instant::now())
        .expect("first verified source must be authorized");
    assert_eq!(first_permit.destination(), first_source);
    let second_permit = registry
        .authorize_uplink(test_id, first_source, 2, std::time::Instant::now())
        .expect("bound source with an increasing sequence must be authorized");
    assert_eq!(second_permit.destination(), first_source);
    assert!(
        registry
            .authorize_uplink(test_id, second_source, 3, std::time::Instant::now())
            .is_none()
    );
    assert!(
        registry
            .authorize_uplink(test_id, first_source, 2, std::time::Instant::now())
            .is_none()
    );
}

#[test]
fn registry_rejects_sequence_zero_without_binding_the_source() {
    let test_id = [0x22; 16];
    let rejected_source = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49152);
    let accepted_source = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49153);
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let registry = SessionRegistry::with_session(test_id, deadline);

    assert!(
        registry
            .authorize_uplink(test_id, rejected_source, 0, std::time::Instant::now())
            .is_none()
    );
    assert!(
        registry
            .authorize_uplink(test_id, accepted_source, 1, std::time::Instant::now())
            .is_some(),
        "an invalid sequence must not bind the session"
    );
}

#[test]
fn registry_rejects_packets_beyond_the_fixed_ceiling() {
    let test_id = [0x23; 16];
    let source = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49152);
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let registry = SessionRegistry::with_session(test_id, deadline);

    for sequence in 1..=MAX_SESSION_PACKETS {
        assert!(
            registry
                .authorize_uplink(test_id, source, sequence, std::time::Instant::now())
                .is_some()
        );
    }
    assert!(
        registry
            .authorize_uplink(
                test_id,
                source,
                MAX_SESSION_PACKETS + 1,
                std::time::Instant::now(),
            )
            .is_none()
    );
}

#[test]
fn concurrent_first_sources_produce_exactly_one_binding() {
    let test_id = [0x33; 16];
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let registry = Arc::new(SessionRegistry::with_session(test_id, deadline));
    let barrier = Arc::new(Barrier::new(3));
    let sources = [
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49152),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49153),
    ];

    let handles: Vec<_> = sources
        .into_iter()
        .map(|source| {
            let registry = Arc::clone(&registry);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                registry.authorize_uplink(test_id, source, 1, std::time::Instant::now())
            })
        })
        .collect();

    barrier.wait();
    let authorizations: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("authorization thread must not panic"))
        .collect();

    assert_eq!(
        authorizations
            .iter()
            .filter(|authorization| authorization.is_some())
            .count(),
        1
    );
}

#[test]
fn unknown_session_is_not_authorized() {
    let registry = SessionRegistry::default();
    let source = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 49152);

    let authorization = registry.authorize(
        [0x11; 16],
        source,
        SystemTime::now() + Duration::from_secs(1),
    );

    assert_eq!(authorization, SessionAuthorization::Unknown);
}

#[test]
fn probe_serves_a_bounded_authenticated_packet_train() {
    let session_id = [0x77; 16];
    let key = [0x22; 32];
    let probe_socket = UdpSocket::bind("127.0.0.1:0").expect("bind loopback probe");
    let probe_address = probe_socket.local_addr().expect("read probe address");
    let session = LocalSession::new(session_id, key, SystemTime::now() + Duration::from_secs(2));
    let probe_thread = thread::spawn(move || serve_session(probe_socket, session));
    let agent_socket = UdpSocket::bind("127.0.0.1:0").expect("bind loopback agent");
    agent_socket
        .set_read_timeout(Some(Duration::from_millis(500)))
        .expect("set receive timeout");

    for sequence in 1..=MAX_SESSION_PACKETS {
        let uplink = Packet::new(session_id, Direction::Uplink, sequence, 0, vec![0_u8; 172]);
        agent_socket
            .send_to(&uplink.encode(&key), probe_address)
            .expect("send authenticated uplink");

        let mut buffer = [0_u8; 1_272];
        let (received, source) = agent_socket
            .recv_from(&mut buffer)
            .expect("receive authenticated downlink");
        assert_eq!(source, probe_address);
        let downlink = Packet::decode(&buffer[..received], &key).expect("decode downlink");
        assert_eq!(downlink.direction(), Direction::Downlink);
        assert_eq!(downlink.sequence_number(), sequence);
    }

    let run = probe_thread
        .join()
        .expect("probe thread must not panic")
        .expect("probe session must complete");
    assert_eq!(run.accepted_packets, MAX_SESSION_PACKETS);
    assert_eq!(run.responses_sent, MAX_SESSION_PACKETS);
}

#[test]
fn malformed_datagram_is_silent_and_does_not_end_the_session() {
    let session_id = [0x70; 16];
    let key = [0x22; 32];
    let probe_socket = UdpSocket::bind("127.0.0.1:0").expect("bind probe");
    let probe_address = probe_socket.local_addr().expect("probe address");
    let session = LocalSession::new(
        session_id,
        key,
        SystemTime::now() + Duration::from_millis(300),
    );
    let probe_thread = thread::spawn(move || serve_session(probe_socket, session));
    let agent_socket = UdpSocket::bind("127.0.0.1:0").expect("bind agent");
    agent_socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .expect("set timeout");

    agent_socket
        .send_to(&[0_u8; 12], probe_address)
        .expect("send malformed datagram");
    let mut buffer = [0_u8; 1_272];
    assert!(agent_socket.recv_from(&mut buffer).is_err());

    let valid = Packet::new(session_id, Direction::Uplink, 1, 0, vec![0_u8; 172]);
    agent_socket
        .send_to(&valid.encode(&key), probe_address)
        .expect("send valid uplink");
    receive_downlink(&agent_socket, &key, 1);

    let run = probe_thread
        .join()
        .expect("probe thread must not panic")
        .expect("probe session must complete");
    assert_eq!(run.accepted_packets, 1);
    assert_eq!(run.responses_sent, 1);
}

#[test]
fn replayed_sequence_is_silent_without_consuming_the_packet_budget() {
    let session_id = [0x71; 16];
    let key = [0x22; 32];
    let probe_socket = UdpSocket::bind("127.0.0.1:0").expect("bind probe");
    let probe_address = probe_socket.local_addr().expect("probe address");
    let session = LocalSession::new(
        session_id,
        key,
        SystemTime::now() + Duration::from_millis(300),
    );
    let probe_thread = thread::spawn(move || serve_session(probe_socket, session));
    let agent_socket = UdpSocket::bind("127.0.0.1:0").expect("bind agent");
    agent_socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .expect("set timeout");

    let first = Packet::new(session_id, Direction::Uplink, 1, 0, vec![0_u8; 172]);
    agent_socket
        .send_to(&first.encode(&key), probe_address)
        .expect("send first uplink");
    receive_downlink(&agent_socket, &key, 1);
    agent_socket
        .send_to(&first.encode(&key), probe_address)
        .expect("send replay");
    let mut buffer = [0_u8; 1_272];
    assert!(agent_socket.recv_from(&mut buffer).is_err());

    let second = Packet::new(session_id, Direction::Uplink, 2, 0, vec![0_u8; 172]);
    agent_socket
        .send_to(&second.encode(&key), probe_address)
        .expect("send increasing uplink");
    receive_downlink(&agent_socket, &key, 2);

    let run = probe_thread
        .join()
        .expect("probe thread must not panic")
        .expect("probe session must complete");
    assert_eq!(run.accepted_packets, 2);
    assert_eq!(run.responses_sent, 2);
}

#[test]
fn rebound_source_is_silent_without_changing_the_bound_source() {
    let session_id = [0x72; 16];
    let key = [0x22; 32];
    let probe_socket = UdpSocket::bind("127.0.0.1:0").expect("bind probe");
    let probe_address = probe_socket.local_addr().expect("probe address");
    let session = LocalSession::new(
        session_id,
        key,
        SystemTime::now() + Duration::from_millis(300),
    );
    let probe_thread = thread::spawn(move || serve_session(probe_socket, session));
    let bound_socket = UdpSocket::bind("127.0.0.1:0").expect("bind first agent");
    let rebound_socket = UdpSocket::bind("127.0.0.1:0").expect("bind second agent");
    bound_socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .expect("set first timeout");
    rebound_socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .expect("set second timeout");

    let first = Packet::new(session_id, Direction::Uplink, 1, 0, vec![0_u8; 172]);
    bound_socket
        .send_to(&first.encode(&key), probe_address)
        .expect("send binding uplink");
    receive_downlink(&bound_socket, &key, 1);

    let second = Packet::new(session_id, Direction::Uplink, 2, 0, vec![0_u8; 172]);
    rebound_socket
        .send_to(&second.encode(&key), probe_address)
        .expect("send rebound uplink");
    let mut buffer = [0_u8; 1_272];
    assert!(rebound_socket.recv_from(&mut buffer).is_err());
    bound_socket
        .send_to(&second.encode(&key), probe_address)
        .expect("send bound-source uplink");
    receive_downlink(&bound_socket, &key, 2);

    let run = probe_thread
        .join()
        .expect("probe thread must not panic")
        .expect("probe session must complete");
    assert_eq!(run.accepted_packets, 2);
    assert_eq!(run.responses_sent, 2);
}

#[test]
fn probe_stops_at_the_fixed_duration_ceiling() {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind probe");
    let session = LocalSession::new(
        [0x73; 16],
        [0x22; 32],
        SystemTime::now() + Duration::from_secs(60),
    );
    let started_at = Instant::now();

    assert_eq!(
        serve_session(socket, session).expect("probe session must complete"),
        ProbeRun::default()
    );
    assert!(started_at.elapsed() <= MAX_SESSION_DURATION + Duration::from_secs(1));
}

#[test]
fn probe_rejects_a_non_loopback_bind_address() {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("bind wildcard test socket");
    let session = LocalSession::new(
        [0x11; 16],
        [0x22; 32],
        SystemTime::now() + Duration::from_secs(2),
    );

    let error = serve_session(socket, session).expect_err("wildcard bind must fail explicitly");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn downlink_direction_sent_to_probe_receives_no_udp_response() {
    let session_id = [0x11; 16];
    let key = [0x22; 32];
    let probe_socket = UdpSocket::bind("127.0.0.1:0").expect("bind loopback probe");
    let probe_address = probe_socket.local_addr().expect("read probe address");
    let session = LocalSession::new(session_id, key, SystemTime::now() + Duration::from_secs(2));
    let probe_thread = thread::spawn(move || serve_session(probe_socket, session));

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
    assert_eq!(
        probe_thread
            .join()
            .expect("probe thread must not panic")
            .expect("probe session must complete"),
        ProbeRun::default()
    );
}

#[test]
fn expired_session_receives_no_udp_response() {
    let session_id = [0x66; 16];
    let key = [0x22; 32];
    let probe_socket = UdpSocket::bind("127.0.0.1:0").expect("bind probe");
    let probe_address = probe_socket.local_addr().expect("probe address");
    let session = LocalSession::new(session_id, key, SystemTime::now() - Duration::from_secs(1));
    let probe_thread = thread::spawn(move || serve_session(probe_socket, session));
    let agent_socket = UdpSocket::bind("127.0.0.1:0").expect("bind agent");
    agent_socket
        .set_read_timeout(Some(Duration::from_millis(150)))
        .expect("set timeout");
    let packet = Packet::new(session_id, Direction::Uplink, 1, 0, vec![0_u8; 172]);
    agent_socket
        .send_to(&packet.encode(&key), probe_address)
        .expect("send expired-session frame");
    let mut buffer = [0_u8; 1_272];
    assert!(
        agent_socket.recv_from(&mut buffer).is_err(),
        "probe must remain silent"
    );
    assert_eq!(
        probe_thread
            .join()
            .expect("probe thread must not panic")
            .expect("probe session must complete"),
        ProbeRun::default()
    );
}

#[test]
fn invalid_hmac_receives_no_udp_response() {
    let session_id = [0x55; 16];
    let key = [0x22; 32];
    let probe_socket = UdpSocket::bind("127.0.0.1:0").expect("bind probe");
    let probe_address = probe_socket.local_addr().expect("probe address");
    let session = LocalSession::new(session_id, key, SystemTime::now() + Duration::from_secs(2));
    let probe_thread = thread::spawn(move || serve_session(probe_socket, session));
    let agent_socket = UdpSocket::bind("127.0.0.1:0").expect("bind agent");
    agent_socket
        .set_read_timeout(Some(Duration::from_millis(150)))
        .expect("set timeout");
    let packet = Packet::new(session_id, Direction::Uplink, 1, 0, vec![0_u8; 172]);
    agent_socket
        .send_to(&packet.encode(&[0x33; 32]), probe_address)
        .expect("send invalid hmac");
    let mut buffer = [0_u8; 1_272];
    assert!(
        agent_socket.recv_from(&mut buffer).is_err(),
        "probe must remain silent"
    );
    assert_eq!(
        probe_thread
            .join()
            .expect("probe thread must not panic")
            .expect("probe session must complete"),
        ProbeRun::default()
    );
}

#[test]
fn unknown_test_id_receives_no_udp_response() {
    let session_id = [0x11; 16];
    let key = [0x22; 32];
    let probe_socket = UdpSocket::bind("127.0.0.1:0").expect("bind loopback probe");
    let probe_address = probe_socket.local_addr().expect("read probe address");
    let session = LocalSession::new(session_id, key, SystemTime::now() + Duration::from_secs(2));
    let probe_thread = thread::spawn(move || serve_session(probe_socket, session));

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
    assert_eq!(
        probe_thread
            .join()
            .expect("probe thread must not panic")
            .expect("probe session must complete"),
        ProbeRun::default()
    );
}
