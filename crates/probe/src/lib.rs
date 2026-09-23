//! Local-only UDP probe and one-time session registry.

use protocol::{Direction, Packet};
use std::net::{SocketAddr, UdpSocket};
use std::time::SystemTime;

/// Result of checking whether an incoming frame may activate a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionAuthorization {
    /// The test ID was never registered, so the probe must remain silent.
    Unknown,
}

/// In-memory local session registry.
#[derive(Default)]
pub struct SessionRegistry;

impl SessionRegistry {
    /// Checks whether a source endpoint may use a test session.
    pub fn authorize(
        &self,
        _test_id: [u8; 16],
        _source: SocketAddr,
        _now: SystemTime,
    ) -> SessionAuthorization {
        SessionAuthorization::Unknown
    }
}

/// One explicit local test session, supplied to the probe at startup.
pub struct LocalSession {
    test_id: [u8; 16],
    key: [u8; 32],
    expires_at: SystemTime,
}

impl LocalSession {
    /// Creates a short-lived session for a loopback-only local run.
    pub fn new(test_id: [u8; 16], key: [u8; 32], expires_at: SystemTime) -> Self {
        Self {
            test_id,
            key,
            expires_at,
        }
    }
}

/// Receives one validated uplink frame and emits one fixed downlink frame.
///
/// Every rejection path returns `false` without sending a UDP response.
pub fn serve_one(socket: UdpSocket, session: LocalSession) -> bool {
    let Ok(bound_address) = socket.local_addr() else {
        return false;
    };
    if !bound_address.ip().is_loopback() {
        return false;
    }

    let mut buffer = [0_u8; 1_272];
    let Ok((received, source)) = socket.recv_from(&mut buffer) else {
        return false;
    };

    if SystemTime::now() >= session.expires_at {
        return false;
    }

    let Ok(packet) = Packet::decode(&buffer[..received], &session.key) else {
        return false;
    };

    if packet.test_id() != session.test_id || packet.direction() != Direction::Uplink {
        return false;
    }

    let response = Packet::new(session.test_id, Direction::Downlink, 0, 0, vec![0_u8; 172]);
    socket
        .send_to(&response.encode(&session.key), source)
        .is_ok()
}
