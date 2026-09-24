use protocol::{Direction, Packet};
use std::fs;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static PORT_LOCK: Mutex<()> = Mutex::new(());

fn available_probe_socket() -> UdpSocket {
    [10_000, 16_384]
        .into_iter()
        .find_map(|port| UdpSocket::bind(("127.0.0.1", port)).ok())
        .expect("one approved local UDP port must be available")
}

fn session_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "nrp-agent-{label}-{}-{}.session",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ))
}

fn write_session(path: &PathBuf, port: u16, expires_at: u64) {
    let contents = format!(
        "NRP-LOCAL-SESSION-V1\n\
         test_id_hex=11111111111111111111111111111111\n\
         hmac_key_hex=2222222222222222222222222222222222222222222222222222222222222222\n\
         expires_at_unix_seconds={expires_at}\n\
         probe_address=127.0.0.1:{port}\n"
    );
    fs::write(path, contents).expect("write temporary session");
}

fn future_expiry() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_secs()
        + 5
}

#[test]
fn agent_binary_completes_and_emits_json() {
    let _port_guard = PORT_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let probe_socket = available_probe_socket();
    let probe_address = probe_socket.local_addr().expect("probe address");
    probe_socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set probe timeout");
    let probe_thread = thread::spawn(move || {
        let key = [0x22; 32];
        for sequence in 1..=32 {
            let mut buffer = [0_u8; 1_272];
            let (received, agent_address) = probe_socket
                .recv_from(&mut buffer)
                .expect("receive agent uplink");
            let uplink = Packet::decode(&buffer[..received], &key).expect("valid uplink");
            assert_eq!(uplink.direction(), Direction::Uplink);
            assert_eq!(uplink.sequence_number(), sequence);
            let response = Packet::new(
                [0x11; 16],
                Direction::Downlink,
                sequence,
                0,
                vec![0_u8; 172],
            );
            probe_socket
                .send_to(&response.encode(&key), agent_address)
                .expect("send downlink");
        }
    });
    let path = session_path("completed");
    write_session(&path, probe_address.port(), future_expiry());

    let output = Command::new(env!("CARGO_BIN_EXE_agent"))
        .args([
            "--session-file",
            path.to_str().expect("UTF-8 temporary path"),
            "--response-timeout-ms",
            "500",
        ])
        .output()
        .expect("run agent binary");
    let _ = fs::remove_file(&path);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 JSON output");
    assert!(stdout.contains("\"component\":\"agent\""));
    assert!(stdout.contains("\"status\":\"COMPLETED\""));
    assert!(stdout.contains("\"uplink_packets_sent\":32"));
    assert!(stdout.contains("\"downlink_packets_received\":32"));
    probe_thread.join().expect("probe thread");
}

#[test]
fn agent_binary_reports_unavailable_udp_as_json() {
    let _port_guard = PORT_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let socket = available_probe_socket();
    let port = socket.local_addr().expect("probe address").port();
    drop(socket);
    let path = session_path("unavailable");
    write_session(&path, port, future_expiry());

    let output = Command::new(env!("CARGO_BIN_EXE_agent"))
        .args([
            "--session-file",
            path.to_str().expect("UTF-8 temporary path"),
            "--response-timeout-ms",
            "25",
        ])
        .output()
        .expect("run agent binary");
    let _ = fs::remove_file(&path);

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 JSON output");
    assert!(stdout.contains("\"status\":\"UDP_UNREACHABLE_OR_BLOCKED\""));
    assert!(stdout.contains("\"downlink_packets_received\":null"));
    assert!(output.stderr.is_empty());
}

#[test]
fn agent_binary_reports_a_partial_exchange_as_an_explicit_error() {
    let _port_guard = PORT_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let fake_probe = available_probe_socket();
    let port = fake_probe.local_addr().expect("probe address").port();
    let responder = thread::spawn(move || {
        let mut buffer = [0_u8; 1_272];
        let (_, agent_address) = fake_probe.recv_from(&mut buffer).expect("receive uplink");
        let response = Packet::new([0x11; 16], Direction::Downlink, 1, 0, vec![0_u8; 172]);
        fake_probe
            .send_to(&response.encode(&[0x22; 32]), agent_address)
            .expect("send one downlink");
        thread::sleep(Duration::from_millis(100));
    });
    let path = session_path("partial");
    write_session(&path, port, future_expiry());

    let output = Command::new(env!("CARGO_BIN_EXE_agent"))
        .args([
            "--session-file",
            path.to_str().expect("UTF-8 temporary path"),
            "--response-timeout-ms",
            "25",
        ])
        .output()
        .expect("run agent binary");
    let _ = fs::remove_file(&path);
    responder.join().expect("responder thread");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .starts_with("agent: local UDP test failed: local exchange incomplete")
    );
}

#[test]
fn agent_binary_does_not_report_a_slow_partial_exchange_as_unreachable() {
    let _port_guard = PORT_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let fake_probe = available_probe_socket();
    let port = fake_probe.local_addr().expect("probe address").port();
    let responder = thread::spawn(move || {
        let key = [0x22; 32];
        let mut buffer = [0_u8; 1_272];
        for sequence in 1..=32 {
            let Ok((_, agent_address)) = fake_probe.recv_from(&mut buffer) else {
                break;
            };
            thread::sleep(Duration::from_millis(75));
            let response = Packet::new(
                [0x11; 16],
                Direction::Downlink,
                sequence,
                0,
                vec![0_u8; 172],
            );
            if fake_probe
                .send_to(&response.encode(&key), agent_address)
                .is_err()
            {
                break;
            }
        }
    });
    let path = session_path("slow-partial");
    write_session(&path, port, future_expiry());

    let output = Command::new(env!("CARGO_BIN_EXE_agent"))
        .args([
            "--session-file",
            path.to_str().expect("UTF-8 temporary path"),
            "--response-timeout-ms",
            "100",
        ])
        .output()
        .expect("run agent binary");
    let _ = fs::remove_file(&path);
    responder.join().expect("responder thread");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .starts_with("agent: local UDP test failed: local exchange incomplete")
    );
}

#[test]
fn agent_binary_rejects_an_invalid_session_explicitly() {
    let path = session_path("invalid");
    fs::write(&path, "not-a-session\n").expect("write invalid session");

    let output = Command::new(env!("CARGO_BIN_EXE_agent"))
        .args([
            "--session-file",
            path.to_str().expect("UTF-8 temporary path"),
        ])
        .output()
        .expect("run agent binary");
    let _ = fs::remove_file(&path);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("agent: invalid local session"));
}
