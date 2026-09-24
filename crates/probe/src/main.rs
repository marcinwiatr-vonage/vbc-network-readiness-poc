use probe::{LocalSession, MAX_SESSION_PACKETS, serve_session};
use protocol::{LOCAL_SESSION_FILE_MAX_BYTES, LocalSessionFile};
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::SystemTime;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("probe: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let session_path = parse_arguments(std::env::args_os().skip(1))?;
    let session_file = read_session(&session_path)?;
    let expires_at = session_file
        .expires_at()
        .map_err(|error| format!("invalid local session: {error}"))?;
    if SystemTime::now() >= expires_at {
        return Err("local session has expired".to_owned());
    }

    let socket = UdpSocket::bind(session_file.probe_address()).map_err(|error| {
        format!(
            "could not bind local probe at {}: {error}",
            session_file.probe_address()
        )
    })?;
    let run = serve_session(
        socket,
        LocalSession::new(session_file.test_id(), session_file.hmac_key(), expires_at),
    )
    .map_err(|error| format!("local probe session failed: {error}"))?;
    let status = if run.accepted_packets == MAX_SESSION_PACKETS
        && run.responses_sent == MAX_SESSION_PACKETS
    {
        "COMPLETED"
    } else {
        "DEADLINE_REACHED"
    };
    println!(
        "{{\"local_run_schema_version\":1,\"component\":\"probe\",\"status\":\"{}\",\"udp_port\":{},\"accepted_packets\":{},\"responses_sent\":{}}}",
        status,
        session_file.probe_address().port(),
        run.accepted_packets,
        run.responses_sent,
    );
    Ok(())
}

fn parse_arguments(mut arguments: impl Iterator<Item = OsString>) -> Result<PathBuf, String> {
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--session-file")) {
        return Err(usage());
    }
    let path = arguments.next().map(PathBuf::from).ok_or_else(usage)?;
    if arguments.next().is_some() {
        return Err(usage());
    }
    Ok(path)
}

fn usage() -> String {
    "usage: probe --session-file <path>".to_owned()
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
