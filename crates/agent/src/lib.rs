//! Local-only endpoint agent.

use protocol::{Direction, Packet};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

/// Counts from one completed local loopback exchange.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalRun {
    pub uplink_sent: u64,
    pub downlink_received: u64,
}

/// Runs one authenticated request/response exchange with a loopback probe.
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

    let uplink = Packet::new(test_id, Direction::Uplink, 1, 0, vec![0_u8; 172]);
    socket.send_to(&uplink.encode(&key), probe_address)?;

    let mut buffer = [0_u8; 1_272];
    let (received, source) = socket.recv_from(&mut buffer)?;
    if source != probe_address {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "response came from an unexpected source",
        ));
    }

    let response = Packet::decode(&buffer[..received], &key)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if response.test_id() != test_id || response.direction() != Direction::Downlink {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "response has an unexpected session or direction",
        ));
    }

    Ok(LocalRun {
        uplink_sent: 1,
        downlink_received: 1,
    })
}
