//! Local-only UDP probe and one-time session registry.

use protocol::{Direction, Packet};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant, SystemTime};

/// Hard ceiling for authenticated uplink packets admitted by one local session.
pub const MAX_SESSION_PACKETS: u32 = 32;
/// Hard monotonic runtime ceiling for one local session.
pub const MAX_SESSION_DURATION: Duration = Duration::from_secs(2);

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
    admitted_packets: u32,
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
                admitted_packets: 0,
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
            || state.admitted_packets >= MAX_SESSION_PACKETS
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
        state.admitted_packets += 1;
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

/// Counts from one bounded local probe loop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProbeRun {
    pub accepted_packets: u32,
    pub responses_sent: u32,
}

/// Runs one loopback-only, authenticated, bounded local probe session.
///
/// Invalid traffic is rejected silently and does not consume the packet budget.
pub fn serve_session(socket: UdpSocket, mut session: LocalSession) -> ProbeRun {
    let started_at = Instant::now();
    session.deadline = session
        .deadline
        .min(checked_deadline(started_at, MAX_SESSION_DURATION));
    serve_bounded(socket, session)
}

fn serve_bounded(socket: UdpSocket, session: LocalSession) -> ProbeRun {
    let mut run = ProbeRun::default();
    let Ok(bound_address) = socket.local_addr() else {
        return run;
    };
    if !bound_address.ip().is_loopback() {
        return run;
    }

    let registry = SessionRegistry::with_session(session.test_id, session.deadline);
    let mut buffer = [0_u8; 1_272];
    while run.accepted_packets < MAX_SESSION_PACKETS {
        let now = Instant::now();
        if now >= session.deadline {
            break;
        }
        if socket
            .set_read_timeout(Some(session.deadline.duration_since(now)))
            .is_err()
        {
            break;
        }

        let (received, source) = match socket.recv_from(&mut buffer) {
            Ok(received) => received,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let Ok(packet) = Packet::decode(&buffer[..received], &session.key) else {
            continue;
        };
        if packet.test_id() != session.test_id || packet.direction() != Direction::Uplink {
            continue;
        }
        let Some(permit) = registry.authorize_uplink(
            packet.test_id(),
            source,
            packet.sequence_number(),
            Instant::now(),
        ) else {
            continue;
        };

        run.accepted_packets += 1;
        let response = Packet::new(
            session.test_id,
            Direction::Downlink,
            run.accepted_packets,
            0,
            vec![0_u8; 172],
        );
        if socket
            .send_to(&response.encode(&session.key), permit.destination())
            .is_ok()
        {
            run.responses_sent += 1;
        }
    }
    run
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
