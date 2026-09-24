//! Local-only endpoint agent.

use protocol::{Direction, Packet};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

/// Fixed packet count for the local Milestone 1 packet train.
pub const LOCAL_PACKET_COUNT: u32 = 32;

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

    let mut run = LocalRun {
        uplink_sent: 0,
        downlink_received: 0,
    };
    let mut buffer = [0_u8; 1_272];
    for sequence in 1..=LOCAL_PACKET_COUNT {
        let uplink = Packet::new(test_id, Direction::Uplink, sequence, 0, vec![0_u8; 172]);
        socket.send_to(&uplink.encode(&key), probe_address)?;
        run.uplink_sent += 1;

        let (received, source) = socket.recv_from(&mut buffer)?;
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
