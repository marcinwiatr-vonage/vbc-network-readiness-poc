//! Local-only UDP probe and one-time session registry.

use protocol::{Direction, Packet};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Instant, SystemTime};

/// Result of checking whether an incoming frame may activate a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionAuthorization {
    /// The test ID was never registered, so the probe must remain silent.
    Unknown,
}

/// In-memory local session registry.
pub struct SessionRegistry {
    sessions: std::sync::Mutex<std::collections::HashMap<[u8; 16], SessionState>>,
}

struct SessionState {
    deadline: std::time::Instant,
    source: Option<SocketAddr>,
    last_sequence: Option<u32>,
}

/// Immutable authorization to send only to the verified source endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SendPermit {
    destination: SocketAddr,
}

impl SendPermit {
    /// Returns the observed source endpoint authorized as the response destination.
    pub const fn destination(self) -> SocketAddr {
        self.destination
    }
}

impl SessionRegistry {
    /// Creates one local session with a monotonic expiry deadline.
    pub fn with_session(test_id: [u8; 16], deadline: std::time::Instant) -> Self {
        let mut sessions = std::collections::HashMap::new();
        sessions.insert(
            test_id,
            SessionState {
                deadline,
                source: None,
                last_sequence: None,
            },
        );
        Self {
            sessions: std::sync::Mutex::new(sessions),
        }
    }

    /// Reports an unknown session without authorizing traffic; retained for the legacy unit test.
    pub fn authorize(
        &self,
        _test_id: [u8; 16],
        _source: SocketAddr,
        _now: SystemTime,
    ) -> SessionAuthorization {
        SessionAuthorization::Unknown
    }

    /// Atomically binds the first source and admits only increasing sequences before expiry.
    pub fn authorize_uplink(
        &self,
        test_id: [u8; 16],
        source: SocketAddr,
        sequence: u32,
        now: std::time::Instant,
    ) -> Option<SendPermit> {
        let mut sessions = self.sessions.lock().ok()?;
        let state = sessions.get_mut(&test_id)?;
        if now >= state.deadline
            || sequence == 0
            || state.last_sequence.is_some_and(|last| sequence <= last)
        {
            return None;
        }
        let destination = match state.source {
            Some(bound) if bound != source => return None,
            Some(bound) => bound,
            None => source,
        };
        state.source = Some(destination);
        state.last_sequence = Some(sequence);
        Some(SendPermit { destination })
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self {
            sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

/// One explicit local test session, supplied to the probe at startup.
pub struct LocalSession {
    test_id: [u8; 16],
    key: [u8; 32],
    deadline: Instant,
}

impl LocalSession {
    /// Creates a short-lived session for a loopback-only local run.
    pub fn new(test_id: [u8; 16], key: [u8; 32], expires_at: SystemTime) -> Self {
        let monotonic_now = Instant::now();
        let remaining = expires_at
            .duration_since(SystemTime::now())
            .unwrap_or_default();
        Self {
            test_id,
            key,
            deadline: checked_deadline(monotonic_now, remaining),
        }
    }
}

fn checked_deadline(now: Instant, remaining: std::time::Duration) -> Instant {
    now.checked_add(remaining).unwrap_or(now)
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

    let registry = SessionRegistry::with_session(session.test_id, session.deadline);
    let mut buffer = [0_u8; 1_272];
    let Ok((received, source)) = socket.recv_from(&mut buffer) else {
        return false;
    };

    if Instant::now() >= session.deadline {
        return false;
    }

    let Ok(packet) = Packet::decode(&buffer[..received], &session.key) else {
        return false;
    };

    if packet.test_id() != session.test_id || packet.direction() != Direction::Uplink {
        return false;
    }
    let Some(permit) = registry.authorize_uplink(
        packet.test_id(),
        source,
        packet.sequence_number(),
        Instant::now(),
    ) else {
        return false;
    };

    let response = Packet::new(session.test_id, Direction::Downlink, 1, 0, vec![0_u8; 172]);
    socket
        .send_to(&response.encode(&session.key), permit.destination())
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::checked_deadline;
    use std::time::{Duration, Instant};

    #[test]
    fn unrepresentable_monotonic_deadline_fails_closed() {
        let now = Instant::now();

        assert_eq!(checked_deadline(now, Duration::MAX), now);
    }
}
