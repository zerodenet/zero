// Mieru protocol metadata encoding — metadata.rs
//
// Two metadata types (32 bytes each):
//   Session metadata — for open/close session messages
//   Data metadata    — for data transfer and ACKs
//
// All multibyte fields are big-endian.

use alloc::vec::Vec;

pub const METADATA_LEN: usize = 32;

// ── Protocol type constants ──────────────────────────────────────────

pub const OPEN_SESSION_REQUEST: u8 = 2;
pub const OPEN_SESSION_RESPONSE: u8 = 3;
pub const CLOSE_SESSION_REQUEST: u8 = 4;
pub const CLOSE_SESSION_RESPONSE: u8 = 5;
pub const DATA_CLIENT_TO_SERVER: u8 = 6;
pub const DATA_SERVER_TO_CLIENT: u8 = 7;
pub const ACK_CLIENT_TO_SERVER: u8 = 8;
pub const ACK_SERVER_TO_CLIENT: u8 = 9;

// ── Session metadata (open / close) ──────────────────────────────────

/// Session metadata used for open/close session messages (32 bytes).
///
/// Layout:
///   [0]    protocol_type   (1 byte)
///   [1]    unused          (1 byte)
///   [2:6]  timestamp       (4 bytes, big-endian minutes since epoch)
///   [6:10] session_id      (4 bytes)
///   [10:14] sequence_number (4 bytes)
///   [14]   status_code     (1 byte)
///   [15:17] payload_length (2 bytes, max 1024)
///   [17]   suffix_length   (1 byte)
///   [18:32] unused         (14 bytes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMetadata {
    pub protocol_type: u8,
    pub timestamp: u32,
    pub session_id: u32,
    pub sequence_number: u32,
    pub status_code: u8,
    pub payload_length: u16,
    pub suffix_length: u8,
}

impl SessionMetadata {
    pub const fn new(protocol_type: u8) -> Self {
        Self {
            protocol_type,
            timestamp: 0,
            session_id: 0,
            sequence_number: 0,
            status_code: 0,
            payload_length: 0,
            suffix_length: 0,
        }
    }

    /// Encode to 32-byte buffer.
    pub fn encode(&self) -> [u8; METADATA_LEN] {
        let mut buf = [0u8; METADATA_LEN];
        buf[0] = self.protocol_type;
        // [1] unused
        buf[2..6].copy_from_slice(&self.timestamp.to_be_bytes());
        buf[6..10].copy_from_slice(&self.session_id.to_be_bytes());
        buf[10..14].copy_from_slice(&self.sequence_number.to_be_bytes());
        buf[14] = self.status_code;
        buf[15..17].copy_from_slice(&self.payload_length.to_be_bytes());
        buf[17] = self.suffix_length;
        // [18..32] unused (already zero)
        buf
    }

    /// Decode from 32-byte slice.
    pub fn decode(buf: &[u8]) -> Self {
        let timestamp = u32::from_be_bytes([buf[2], buf[3], buf[4], buf[5]]);
        let session_id = u32::from_be_bytes([buf[6], buf[7], buf[8], buf[9]]);
        let sequence_number = u32::from_be_bytes([buf[10], buf[11], buf[12], buf[13]]);
        let payload_length = u16::from_be_bytes([buf[15], buf[16]]);

        Self {
            protocol_type: buf[0],
            timestamp,
            session_id,
            sequence_number,
            status_code: buf[14],
            payload_length,
            suffix_length: buf[17],
        }
    }
}

// ── Data metadata (transfer / ACK) ───────────────────────────────────

/// Data metadata used for data transfer and ACK messages (32 bytes).
///
/// Layout:
///   [0]    protocol_type   (1 byte)
///   [1]    unused          (1 byte)
///   [2:6]  timestamp       (4 bytes)
///   [6:10] session_id      (4 bytes)
///   [10:14] sequence_number (4 bytes)
///   [14:18] unack_sequence  (4 bytes)
///   [18:20] window_size    (2 bytes)
///   [20]   fragment_number (1 byte)
///   [21]   prefix_length   (1 byte)
///   [22:24] payload_length (2 bytes)
///   [24]   suffix_length   (1 byte)
///   [25:32] unused         (7 bytes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataMetadata {
    pub protocol_type: u8,
    pub timestamp: u32,
    pub session_id: u32,
    pub sequence_number: u32,
    pub unack_sequence: u32,
    pub window_size: u16,
    pub fragment_number: u8,
    pub prefix_length: u8,
    pub payload_length: u16,
    pub suffix_length: u8,
}

impl DataMetadata {
    pub const fn new(protocol_type: u8) -> Self {
        Self {
            protocol_type,
            timestamp: 0,
            session_id: 0,
            sequence_number: 0,
            unack_sequence: 0,
            window_size: 0,
            fragment_number: 0,
            prefix_length: 0,
            payload_length: 0,
            suffix_length: 0,
        }
    }

    /// Encode to 32-byte buffer.
    pub fn encode(&self) -> [u8; METADATA_LEN] {
        let mut buf = [0u8; METADATA_LEN];
        buf[0] = self.protocol_type;
        // [1] unused
        buf[2..6].copy_from_slice(&self.timestamp.to_be_bytes());
        buf[6..10].copy_from_slice(&self.session_id.to_be_bytes());
        buf[10..14].copy_from_slice(&self.sequence_number.to_be_bytes());
        buf[14..18].copy_from_slice(&self.unack_sequence.to_be_bytes());
        buf[18..20].copy_from_slice(&self.window_size.to_be_bytes());
        buf[20] = self.fragment_number;
        buf[21] = self.prefix_length;
        buf[22..24].copy_from_slice(&self.payload_length.to_be_bytes());
        buf[24] = self.suffix_length;
        // [25..32] unused (already zero)
        buf
    }

    /// Decode from 32-byte slice.
    pub fn decode(buf: &[u8]) -> Self {
        let timestamp = u32::from_be_bytes([buf[2], buf[3], buf[4], buf[5]]);
        let session_id = u32::from_be_bytes([buf[6], buf[7], buf[8], buf[9]]);
        let sequence_number = u32::from_be_bytes([buf[10], buf[11], buf[12], buf[13]]);
        let unack_sequence = u32::from_be_bytes([buf[14], buf[15], buf[16], buf[17]]);
        let window_size = u16::from_be_bytes([buf[18], buf[19]]);
        let payload_length = u16::from_be_bytes([buf[22], buf[23]]);

        Self {
            protocol_type: buf[0],
            timestamp,
            session_id,
            sequence_number,
            unack_sequence,
            window_size,
            fragment_number: buf[20],
            prefix_length: buf[21],
            payload_length,
            suffix_length: buf[24],
        }
    }
}
