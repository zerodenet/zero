//! Zero-allocation sliding buffer for streaming data
//!
//! This module provides a fixed-capacity sliding buffer optimized for
//! streaming protocols where data is written at one end and read from another.
//! Unlike a true ring buffer, this uses a linear layout with lazy compaction
//! via `copy_within()`, which is optimal for use cases requiring contiguous
//! slices (like TLS record processing).
//!
//! # Design Rationale
//!
//! A true ring buffer wraps around, but that creates non-contiguous data
//! regions which require either two-slice APIs or copies for consumers
//! expecting `&[u8]`. Since TLS encryption/decryption functions need
//! contiguous slices, this linear design with lazy compaction is more
//! efficient for our use case.

use std::io::{BufRead, Read};

/// A fixed-capacity sliding buffer with zero-allocation read/write operations.
///
/// This buffer maintains start and end offsets into a pre-allocated storage,
/// allowing efficient append and consume operations without allocating.
/// When the write end approaches capacity, `compact()` or `maybe_compact()`
/// can be used to move remaining data to the front using `copy_within()`.
///
/// # Example
/// ```ignore
/// let mut buf = SlideBuffer::new(1024);
/// buf.extend_from_slice(b"hello");
/// assert_eq!(buf.as_slice(), b"hello");
/// buf.consume(2);
/// assert_eq!(buf.as_slice(), b"llo");
/// ```
pub struct SlideBuffer {
    /// Pre-allocated buffer storage
    data: Box<[u8]>,
    /// Start offset of valid data (inclusive)
    start: usize,
    /// End offset of valid data (exclusive)
    end: usize,
}

impl SlideBuffer {
    /// Create a new slide buffer with the specified capacity.
    ///
    /// The buffer is allocated once and reused for all operations.
    #[inline]
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![0_u8; capacity].into_boxed_slice(),
            start: 0,
            end: 0,
        }
    }

    /// Returns the number of bytes currently stored in the buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Returns true if the buffer contains no data.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Returns the remaining space available for writing.
    ///
    /// This is the space at the end of the buffer. To reclaim space
    /// consumed from the front, call `compact()`.
    #[inline]
    pub fn remaining_capacity(&self) -> usize {
        self.data.len() - self.end
    }

    /// Get a slice of the readable data.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[self.start..self.end]
    }

    /// Get a mutable slice for writing new data at the end.
    ///
    /// Returns the writable portion at the end of the buffer.
    /// After writing, call `advance_write(n)` to mark bytes as written.
    ///
    /// # Example
    /// ```ignore
    /// let mut buf = SlideBuffer::new(1024);
    /// let write_buf = buf.write_slice();
    /// write_buf[..5].copy_from_slice(b"hello");
    /// buf.advance_write(5);
    /// ```
    #[inline]
    pub fn write_slice(&mut self) -> &mut [u8] {
        &mut self.data[self.end..]
    }

    /// Extend the buffer with data from a slice.
    ///
    /// # Panics
    /// Panics in debug mode if there isn't enough capacity.
    #[inline]
    pub fn extend_from_slice(&mut self, data: &[u8]) {
        debug_assert!(
            self.remaining_capacity() >= data.len(),
            "SlideBuffer overflow: need {} bytes, have {}",
            data.len(),
            self.remaining_capacity()
        );
        let end = self.end;
        self.data[end..end + data.len()].copy_from_slice(data);
        self.end += data.len();
    }

    /// Mark n bytes as written (after writing to `write_slice()`).
    #[inline]
    pub fn advance_write(&mut self, n: usize) {
        debug_assert!(
            self.end + n <= self.data.len(),
            "SlideBuffer advance_write overflow: end={}, n={}, capacity={}",
            self.end,
            n,
            self.data.len()
        );
        self.end += n;
    }

    /// Consume n bytes from the front of the buffer.
    ///
    /// # Panics
    /// Panics in debug mode if n exceeds the available data.
    #[inline]
    pub fn consume(&mut self, n: usize) {
        debug_assert!(
            n <= self.len(),
            "SlideBuffer consume underflow: n={}, len={}",
            n,
            self.len()
        );
        self.start += n;

        // Reset offsets if buffer is now empty
        if self.start >= self.end {
            self.start = 0;
            self.end = 0;
        }
    }

    /// Compact the buffer by moving data to the front.
    ///
    /// This reclaims the space that was consumed from the front,
    /// making it available for writing at the end.
    /// Uses `copy_within()` which is optimized by the compiler.
    #[inline]
    pub fn compact(&mut self) {
        if self.start > 0 && self.start < self.end {
            self.data.copy_within(self.start..self.end, 0);
            self.end -= self.start;
            self.start = 0;
        } else if self.start >= self.end {
            self.start = 0;
            self.end = 0;
        }
    }

    /// Compact only if we've consumed more than the threshold.
    ///
    /// This amortizes the cost of compaction over many operations,
    /// avoiding unnecessary copies when little space would be reclaimed.
    #[inline]
    pub fn maybe_compact(&mut self, threshold: usize) {
        if self.start > threshold {
            self.compact();
        }
    }

    /// Returns a two-byte value at the given offset as big-endian u16.
    #[inline]
    pub fn get_u16_be(&self, offset: usize) -> Option<u16> {
        if offset + 2 <= self.len() {
            let idx = self.start + offset;
            Some(u16::from_be_bytes([self.data[idx], self.data[idx + 1]]))
        } else {
            None
        }
    }

    /// Get a mutable slice of the readable data for in-place modification.
    ///
    /// This allows callers to modify data in-place (e.g., for decryption)
    /// without copying to a separate buffer.
    #[inline]
    pub fn slice_mut(&mut self, range: std::ops::Range<usize>) -> &mut [u8] {
        &mut self.data[self.start + range.start..self.start + range.end]
    }
}

impl Read for SlideBuffer {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let available = self.len();
        if available == 0 {
            return Ok(0);
        }
        let to_read = buf.len().min(available);
        buf[..to_read].copy_from_slice(&self.data[self.start..self.start + to_read]);
        self.consume(to_read);
        Ok(to_read)
    }
}

impl BufRead for SlideBuffer {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        Ok(self.as_slice())
    }

    fn consume(&mut self, amt: usize) {
        SlideBuffer::consume(self, amt);
    }
}

impl std::ops::Index<usize> for SlideBuffer {
    type Output = u8;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.data[self.start + index]
    }
}

impl std::ops::Index<std::ops::Range<usize>> for SlideBuffer {
    type Output = [u8];

    #[inline]
    fn index(&self, range: std::ops::Range<usize>) -> &Self::Output {
        &self.data[self.start + range.start..self.start + range.end]
    }
}

impl std::ops::Index<std::ops::RangeFrom<usize>> for SlideBuffer {
    type Output = [u8];

    #[inline]
    fn index(&self, range: std::ops::RangeFrom<usize>) -> &Self::Output {
        &self.data[self.start + range.start..self.end]
    }
}

impl std::ops::Index<std::ops::RangeTo<usize>> for SlideBuffer {
    type Output = [u8];

    #[inline]
    fn index(&self, range: std::ops::RangeTo<usize>) -> &Self::Output {
        &self.data[self.start..self.start + range.end]
    }
}
