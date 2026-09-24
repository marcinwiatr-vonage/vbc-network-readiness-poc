use agent::run_local_test;
use probe::{LocalSession, MAX_SESSION_PACKETS, serve_session};
use protocol::{Direction, Packet};
use std::net::UdpSocket;
use std::thread;
use std::time::{Duration, SystemTime};

#[test]
fn agent_rejects_an_uplink_direction_response() {
    let test_id = [0x11; 16];
    let key = [0x22; 32];
    let fake_probe = UdpSocket::bind("127.0.0.1:0").expect("bind fake probe");
    let probe_address = fake_probe.local_addr().expect("read probe address");
    let responder = thread::spawn(move || {
        let mut buffer = [0_u8; 1_272];
        let (_, agent_address) = fake_probe
            .recv_from(&mut buffer)
            .expect("receive agent packet");
        let wrong_direction = Packet::new(test_id, Direction::Uplink, 0, 0, vec![0_u8; 172]);
        fake_probe
            .send_to(&wrong_direction.encode(&key), agent_address)
            .expect("send wrong-direction response");
    });

    let error = run_local_test(probe_address, test_id, key, Duration::from_millis(500))
        .expect_err("agent must reject an uplink response");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    responder.join().expect("responder thread must not panic");
}

#[test]
fn agent_completes_an_authenticated_loopback_exchange() {
    let test_id = [0x11; 16];
    let key = [0x22; 32];
    let probe_socket = UdpSocket::bind("127.0.0.1:0").expect("bind loopback probe");
    let probe_address = probe_socket.local_addr().expect("read probe address");
    let session = LocalSession::new(test_id, key, SystemTime::now() + Duration::from_secs(2));

    let probe_thread = thread::spawn(move || serve_session(probe_socket, session));
    let result = run_local_test(probe_address, test_id, key, Duration::from_millis(500))
        .expect("agent must receive a valid local response");

    assert_eq!(result.uplink_sent, u64::from(MAX_SESSION_PACKETS));
    assert_eq!(result.downlink_received, u64::from(MAX_SESSION_PACKETS));
    let probe_run = probe_thread
        .join()
        .expect("probe thread must not panic")
        .expect("probe session must complete");
    assert_eq!(probe_run.accepted_packets, MAX_SESSION_PACKETS);
    assert_eq!(probe_run.responses_sent, MAX_SESSION_PACKETS);
}
