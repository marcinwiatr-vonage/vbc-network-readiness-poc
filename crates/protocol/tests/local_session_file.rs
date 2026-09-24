use protocol::LocalSessionFile;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

const VALID_SESSION: &str = "NRP-LOCAL-SESSION-V1\n\
test_id_hex=11111111111111111111111111111111\n\
hmac_key_hex=2222222222222222222222222222222222222222222222222222222222222222\n\
expires_at_unix_seconds=2000000000\n\
probe_address=127.0.0.1:10000\n";

#[test]
fn canonical_local_session_file_parses() {
    let session = LocalSessionFile::parse(VALID_SESSION.as_bytes()).expect("canonical session");

    assert_eq!(session.test_id(), [0x11; 16]);
    assert_eq!(session.hmac_key(), [0x22; 32]);
    assert_eq!(session.expires_at_unix_seconds(), 2_000_000_000);
    assert_eq!(
        session.probe_address(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 10_000)
    );
}

#[test]
fn noncanonical_or_unsafe_local_session_files_are_rejected() {
    let cases = [
        VALID_SESSION.replace("1111", "AAAA"),
        VALID_SESSION.replace(
            "expires_at_unix_seconds=2000000000",
            "expires_at_unix_seconds=02000000000",
        ),
        VALID_SESSION.replace(
            "expires_at_unix_seconds=2000000000",
            "expires_at_unix_seconds=+2000000000",
        ),
        VALID_SESSION.replace("127.0.0.1:10000", "0.0.0.0:10000"),
        VALID_SESSION.replace("127.0.0.1:10000", "127.0.0.1:9999"),
        VALID_SESSION.replace("127.0.0.1:10000", "127.0.0.1:010000"),
        VALID_SESSION.replace("test_id_hex=", "unknown="),
        format!("{VALID_SESSION}trailing=true\n"),
        VALID_SESSION.replace('\n', "\r\n"),
        "x".repeat(1_025),
    ];

    for invalid in cases {
        assert!(
            LocalSessionFile::parse(invalid.as_bytes()).is_err(),
            "invalid session unexpectedly parsed"
        );
    }
}
