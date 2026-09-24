use agent::run_local_test;
use protocol::{LOCAL_SESSION_FILE_MAX_BYTES, LocalSessionFile, PROTOCOL_VERSION};
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

const DEFAULT_RESPONSE_TIMEOUT_MS: u64 = 500;
const MAX_RESPONSE_TIMEOUT_MS: u64 = 2_000;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("agent: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let (session_path, response_timeout) = parse_arguments(std::env::args_os().skip(1))?;
    let session = read_session(&session_path)?;
    let expires_at = session
        .expires_at()
        .map_err(|error| format!("invalid local session: {error}"))?;
    if SystemTime::now() >= expires_at {
        return Err("local session has expired".to_owned());
    }

    match run_local_test(
        session.probe_address(),
        session.test_id(),
        session.hmac_key(),
        response_timeout,
    ) {
        Ok(result) => {
            println!(
                "{{\"local_run_schema_version\":1,\"component\":\"agent\",\"status\":\"COMPLETED\",\"protocol_version\":{},\"test_id\":\"{}\",\"udp_port\":{},\"uplink_packets_sent\":{},\"downlink_packets_received\":{}}}",
                PROTOCOL_VERSION,
                hex_test_id(session.test_id()),
                session.probe_address().port(),
                result.uplink_sent,
                result.downlink_received,
            );
            Ok(ExitCode::SUCCESS)
        }
        Err(error) if is_unavailable_error(error.kind()) => {
            println!(
                "{{\"local_run_schema_version\":1,\"component\":\"agent\",\"status\":\"UDP_UNREACHABLE_OR_BLOCKED\",\"protocol_version\":{},\"test_id\":\"{}\",\"udp_port\":{},\"uplink_packets_sent\":null,\"downlink_packets_received\":null}}",
                PROTOCOL_VERSION,
                hex_test_id(session.test_id()),
                session.probe_address().port(),
            );
            Ok(ExitCode::from(2))
        }
        Err(error) => Err(format!("local UDP test failed: {error}")),
    }
}

fn parse_arguments(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<(PathBuf, Duration), String> {
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--session-file")) {
        return Err(usage());
    }
    let session_path = arguments.next().map(PathBuf::from).ok_or_else(usage)?;
    let mut timeout_ms = DEFAULT_RESPONSE_TIMEOUT_MS;
    if let Some(argument) = arguments.next() {
        if argument != "--response-timeout-ms" {
            return Err(usage());
        }
        timeout_ms = arguments
            .next()
            .and_then(|value| value.to_str().and_then(|value| value.parse().ok()))
            .filter(|value| (1..=MAX_RESPONSE_TIMEOUT_MS).contains(value))
            .ok_or_else(|| {
                format!("response timeout must be between 1 and {MAX_RESPONSE_TIMEOUT_MS} ms")
            })?;
    }
    if arguments.next().is_some() {
        return Err(usage());
    }
    Ok((session_path, Duration::from_millis(timeout_ms)))
}

fn usage() -> String {
    "usage: agent --session-file <path> [--response-timeout-ms <1..=2000>]".to_owned()
}

fn read_session(path: &Path) -> Result<LocalSessionFile, String> {
    let file = File::open(path)
        .map_err(|error| format!("could not open session file {}: {error}", path.display()))?;
    let mut bytes = Vec::with_capacity(LOCAL_SESSION_FILE_MAX_BYTES + 1);
    file.take((LOCAL_SESSION_FILE_MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read session file {}: {error}", path.display()))?;
    LocalSessionFile::parse(&bytes).map_err(|error| format!("invalid local session: {error}"))
}

fn is_unavailable_error(kind: io::ErrorKind) -> bool {
    matches!(
        kind,
        io::ErrorKind::WouldBlock
            | io::ErrorKind::TimedOut
            | io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
    )
}

fn hex_test_id(test_id: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(32);
    for byte in test_id {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}
