use protocol::{Direction, Packet};
use std::fs;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn available_port() -> u16 {
    [10_000, 16_384]
        .into_iter()
        .find(|port| UdpSocket::bind(("127.0.0.1", *port)).is_ok())
        .expect("one approved local UDP port must be available")
}

fn session_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "nrp-probe-{label}-{}-{}.session",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ))
}

fn write_session(path: &PathBuf, port: u16) {
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_secs()
        + 5;
    let contents = format!(
        "NRP-LOCAL-SESSION-V1\n\
         test_id_hex=11111111111111111111111111111111\n\
         hmac_key_hex=2222222222222222222222222222222222222222222222222222222222222222\n\
         expires_at_unix_seconds={expires_at}\n\
         probe_address=127.0.0.1:{port}\n"
    );
    fs::write(path, contents).expect("write temporary session");
}

#[test]
fn probe_binary_serves_a_process_level_exchange_and_emits_json() {
    let port = available_port();
    let path = session_path("completed");
    write_session(&path, port);
    let child = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args([
            "--session-file",
            path.to_str().expect("UTF-8 temporary path"),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start probe binary");
    thread::sleep(Duration::from_millis(150));

    let client = UdpSocket::bind("127.0.0.1:0").expect("bind client");
    client
        .set_read_timeout(Some(Duration::from_millis(500)))
        .expect("set timeout");
    let target = ("127.0.0.1", port);
    let key = [0x22; 32];
    for sequence in 1..=32 {
        let packet = Packet::new([0x11; 16], Direction::Uplink, sequence, 0, vec![0_u8; 172]);
        client
            .send_to(&packet.encode(&key), target)
            .expect("send uplink");
        let mut buffer = [0_u8; 1_272];
        let (received, _) = client.recv_from(&mut buffer).expect("receive downlink");
        let response = Packet::decode(&buffer[..received], &key).expect("valid downlink");
        assert_eq!(response.direction(), Direction::Downlink);
        assert_eq!(response.sequence_number(), sequence);
    }

    let output = child.wait_with_output().expect("wait for probe");
    let _ = fs::remove_file(&path);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 JSON output");
    assert!(stdout.contains("\"component\":\"probe\""));
    assert!(stdout.contains("\"status\":\"COMPLETED\""));
    assert!(stdout.contains("\"accepted_packets\":32"));
    assert!(stdout.contains("\"responses_sent\":32"));
}

#[test]
fn probe_binary_rejects_an_invalid_session_explicitly() {
    let path = session_path("invalid");
    fs::write(&path, "not-a-session\n").expect("write invalid session");

    let output = Command::new(env!("CARGO_BIN_EXE_probe"))
        .args([
            "--session-file",
            path.to_str().expect("UTF-8 temporary path"),
        ])
        .output()
        .expect("run probe binary");
    let _ = fs::remove_file(&path);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("probe: invalid local session"));
}
