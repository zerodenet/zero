// Mieru protocol session state — session.rs
//
// Tracks a single mieru session lifecycle:
//   New → OpenWait → Established → CloseWait → Closed

/// Session state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Waiting for openSessionResponse from server.
    OpenWait,
    /// Session established, data can flow.
    Established,
    /// closeSessionRequest sent, waiting for closeSessionResponse.
    CloseWait,
    /// Session closed.
    Closed,
}

/// A mieru session tracked by the client.
#[derive(Debug)]
pub struct MieruSession {
    pub session_id: u32,
    pub state: SessionState,
    /// Next sequence number for outbound data segments.
    pub send_seq: u32,
    /// Expected next sequence number from peer.
    pub recv_seq: u32,
    /// Peer's latest unacknowledged sequence number.
    pub peer_unack: u32,
    /// Peer's advertised window size.
    pub peer_window: u16,
}

impl MieruSession {
    /// Create a new session with a random session ID.
    pub fn new() -> Self {
        // Simple session ID from system time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let session_id = (now.as_nanos() & 0xFFFF_FFFF) as u32;

        Self {
            session_id,
            state: SessionState::OpenWait,
            send_seq: 0,
            recv_seq: 0,
            peer_unack: 0,
            peer_window: 1024,
        }
    }

    /// Create a session with a specific ID (server-side).
    pub fn with_id(session_id: u32) -> Self {
        Self {
            session_id,
            state: SessionState::OpenWait,
            send_seq: 0,
            recv_seq: 0,
            peer_unack: 0,
            peer_window: 1024,
        }
    }

    /// Mark session as established.
    pub fn established(&mut self) {
        self.state = SessionState::Established;
    }

    /// Initiate close.
    pub fn close(&mut self) {
        self.state = SessionState::CloseWait;
    }

    /// Confirm close.
    pub fn closed(&mut self) {
        self.state = SessionState::Closed;
    }

    /// Advance send sequence and return the current value.
    pub fn next_send_seq(&mut self) -> u32 {
        let seq = self.send_seq;
        self.send_seq = self.send_seq.wrapping_add(1);
        seq
    }

    /// Current timestamp in minutes since epoch.
    pub fn timestamp_minutes() -> u32 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        (now.as_secs() / 60) as u32
    }
}
