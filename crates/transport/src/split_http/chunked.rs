use std::io;

use tokio::io::ReadBuf;

/// HTTP chunked-transfer-encoding decoder state machine.
///
/// Pure state over an internal byte buffer — it performs no I/O. The owner
/// feeds raw bytes via [`ChunkedDecoder::feed`] and drains decoded body bytes
/// via [`ChunkedDecoder::try_decode`]. This correctly handles arbitrary TCP
/// segmentation and consumes the trailing `\r\n` after each chunk's data
/// (the bug the original two-connection decoder had: it parsed the `\r\n`
/// terminator as a size line and broke on multi-chunk responses).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ChunkState {
    /// Reading the `<hex>[;ext]\r\n` chunk-size line.
    Size,
    /// Reading `chunk_remaining` bytes of chunk data.
    Data,
    /// Consuming the trailing `\r\n` after chunk data.
    Trailer,
}

/// Outcome of a single [`ChunkedDecoder::try_decode`] pass.
pub(super) enum DecodeStep {
    /// Body bytes were produced, the stream hit EOF, or the output buffer is
    /// full — the caller returns `Poll::Ready(Ok(()))`.
    Done,
    /// More raw bytes are required — the caller feeds its source, then retries.
    NeedsMore,
}

pub(super) struct ChunkedDecoder {
    /// Buffered raw bytes not yet consumed by the decoder.
    raw: Vec<u8>,
    /// Consumed offset within `raw`.
    raw_pos: usize,
    state: ChunkState,
    /// Bytes remaining in the current data chunk.
    chunk_remaining: usize,
    /// Set once the terminating `0` chunk has been seen.
    eof: bool,
}

impl ChunkedDecoder {
    pub(super) fn new() -> Self {
        Self {
            raw: Vec::new(),
            raw_pos: 0,
            state: ChunkState::Size,
            chunk_remaining: 0,
            eof: false,
        }
    }

    /// Build a decoder pre-seeded with bytes already read past a header
    /// boundary (e.g. response body bytes captured during the handshake).
    pub(super) fn with_prefetched(prefetched: Vec<u8>) -> Self {
        Self {
            raw: prefetched,
            raw_pos: 0,
            state: ChunkState::Size,
            chunk_remaining: 0,
            eof: false,
        }
    }

    pub(super) fn feed(&mut self, bytes: &[u8]) {
        self.raw.extend_from_slice(bytes);
    }

    /// Drop consumed bytes from `raw` once fully drained.
    fn compact(&mut self) {
        if self.raw_pos >= self.raw.len() {
            self.raw.clear();
            self.raw_pos = 0;
        }
    }

    /// Try to decode body bytes into `buf`.
    ///
    /// Returns `Done` when output was produced, the stream hit EOF, or the
    /// output buffer is full; returns `NeedsMore` only when **nothing** was
    /// produced this pass and more raw bytes are required. This respects the
    /// `AsyncRead` contract — a caller must never return `Pending` while it
    /// has already filled the caller's buffer, otherwise the peer waits
    /// forever for an ack that never comes (a real deadlock the greedy
    /// version caused in the stream-one round-trip).
    pub(super) fn try_decode(&mut self, buf: &mut ReadBuf<'_>) -> io::Result<DecodeStep> {
        if self.eof {
            return Ok(DecodeStep::Done);
        }
        let mut produced = false;
        loop {
            self.compact();
            match self.state {
                ChunkState::Size => {
                    let window = &self.raw[self.raw_pos..];
                    if let Some(rel) = window.windows(2).position(|w| w == b"\r\n") {
                        let line = &self.raw[self.raw_pos..self.raw_pos + rel];
                        self.raw_pos += rel + 2;
                        let hex_str = std::str::from_utf8(line).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "split-http: non-UTF-8 chunk size",
                            )
                        })?;
                        // RFC 7230 chunk-size is hex digits, optionally followed
                        // by `;chunk-ext` — ignore any extension before parsing.
                        let hex_part = hex_str.split(';').next().unwrap_or("").trim();
                        let size = usize::from_str_radix(hex_part, 16).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("split-http: bad chunk-size: {hex_str}"),
                            )
                        })?;
                        if size == 0 {
                            self.eof = true;
                            return Ok(DecodeStep::Done);
                        }
                        self.chunk_remaining = size;
                        self.state = ChunkState::Data;
                        continue;
                    }
                    return Ok(if produced {
                        DecodeStep::Done
                    } else {
                        DecodeStep::NeedsMore
                    });
                }
                ChunkState::Data => {
                    let avail = self.raw.len() - self.raw_pos;
                    if avail == 0 {
                        return Ok(if produced {
                            DecodeStep::Done
                        } else {
                            DecodeStep::NeedsMore
                        });
                    }
                    if buf.remaining() == 0 {
                        return Ok(DecodeStep::Done);
                    }
                    let n = avail.min(self.chunk_remaining).min(buf.remaining());
                    buf.put_slice(&self.raw[self.raw_pos..self.raw_pos + n]);
                    self.raw_pos += n;
                    self.chunk_remaining -= n;
                    produced = true;
                    if self.chunk_remaining == 0 {
                        self.state = ChunkState::Trailer;
                    }
                    if buf.remaining() == 0 {
                        return Ok(DecodeStep::Done);
                    }
                    continue;
                }
                ChunkState::Trailer => {
                    let avail = self.raw.len() - self.raw_pos;
                    if avail < 2 {
                        return Ok(if produced {
                            DecodeStep::Done
                        } else {
                            DecodeStep::NeedsMore
                        });
                    }
                    // Consume the trailing `\r\n` after chunk data.
                    self.raw_pos += 2;
                    self.state = ChunkState::Size;
                    continue;
                }
            }
        }
    }
}
