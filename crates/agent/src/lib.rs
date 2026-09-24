//! Local-only endpoint agent.

use protocol::{Direction, Packet};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// Fixed packet count for the local Milestone 1 packet train.
pub const LOCAL_PACKET_COUNT: u32 = 32;
/// Hard monotonic runtime ceiling for one local agent run.
pub const LOCAL_TEST_MAX_DURATION: Duration = Duration::from_secs(2);

/// Counts from one completed local loopback exchange.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalRun {
    pub uplink_sent: u64,
    pub downlink_received: u64,
}

/// Runs one bounded authenticated packet train with a loopback probe.
pub fn run_local_test(
    probe_address: SocketAddr,
    test_id: [u8; 16],
    key: [u8; 32],
    response_timeout: Duration,
) -> io::Result<LocalRun> {
    if !probe_address.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local test requires a loopback probe address",
        ));
    }

    let socket = UdpSocket::bind("127.0.0.1:0")?;
    socket.set_read_timeout(Some(response_timeout))?;
    let deadline = Instant::now()
        .checked_add(LOCAL_TEST_MAX_DURATION)
        .ok_or_else(|| io::Error::other("could not represent local test deadline"))?;

    let mut run = LocalRun {
        uplink_sent: 0,
        downlink_received: 0,
    };
    let mut buffer = [0_u8; 1_272];
    for sequence in 1..=LOCAL_PACKET_COUNT {
        let now = Instant::now();
        if now >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "local test exceeded its two-second duration ceiling",
            ));
        }
        socket.set_read_timeout(Some(response_timeout.min(deadline.duration_since(now))))?;
        let uplink = Packet::new(test_id, Direction::Uplink, sequence, 0, vec![0_u8; 172]);
        socket.send_to(&uplink.encode(&key), probe_address)?;
        run.uplink_sent += 1;

        let (received, source) = match socket.recv_from(&mut buffer) {
            Ok(response) => response,
            Err(error) if run.downlink_received == 0 => return Err(error),
            Err(error) => {
                return Err(io::Error::other(format!(
                    "local exchange incomplete after {} authenticated responses: {error}",
                    run.downlink_received
                )));
            }
        };
        if source != probe_address {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "response came from an unexpected source",
            ));
        }

        let response = Packet::decode(&buffer[..received], &key)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if response.test_id() != test_id
            || response.direction() != Direction::Downlink
            || response.sequence_number() != sequence
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "response has an unexpected session, direction, or sequence",
            ));
        }
        run.downlink_received += 1;
    }

    Ok(run)
}
