//! # Ring buffer tools.
use core::{fmt, ops::Index};
use bytemuck::Zeroable;

/// A ring buffer trait.
#[allow(dead_code)]
pub trait RingBuffer: Default + Zeroable + Index<usize, Output=u8> {
    /// The size of the buffer in bytes.
    const BUFFER_SIZE: usize;
    /// Initialize the ring buffer by filling it up with the given `byte`.
    fn initialize(&mut self, byte: u8);
    /// Initialize the ring buffer using a custom function that should fill
    /// it with predetermined values.
    fn initialize_with(&mut self, init: impl FnOnce(&mut [u8]));
    /// The current value of the internal cursor.
    fn cursor(&self) -> usize;
    /// Allows to set the current value of the internal cursor.
    fn set_cursor(&mut self, pos: isize);
    /// Pushes the new byte value to the buffer, overwriting the oldest one.
    fn push(&mut self, byte: u8);
    /// Returns an iterator which will yield consecutive bytes from the buffer starting at `-offset`
    /// from the last element.
    ///
    /// `offset` = 0 indicates the last element written to the buffer.
    ///
    /// At each iteration the yielded value is also being pushed to the ring buffer.
    fn iter_from_offset(&mut self, offset: usize) -> HistoryIter<'_, Self>;
    /// Returns an iterator which will yield consecutive bytes from the buffer starting at
    /// an absolute index `pos`.
    ///
    /// This method ignores the current cursor of the buffer.
    ///
    /// At each iteration the yielded value is also being pushed to the ring buffer.
    fn iter_from_pos(&mut self, pos: usize) -> HistoryIter<'_, Self>;
}

/// A generic ring buffer implementation using arrays of the size of the power of two as internal buffers.
///
/// `N` must be a power of 2.
#[derive(Clone, Zeroable)]
pub struct RingArrayBuf<const N: usize> {
    buffer: [u8; N],
    cursor: usize
}

/// The ring buffer history iterator.
pub struct HistoryIter<'a, T> {
    index: usize,
    ringbuf: &'a mut T
}

impl<const N: usize> fmt::Debug for RingArrayBuf<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RingArrayBuf")
         .field("buffer", &N)
         .field("cursor", &self.cursor)
         .finish()
    }
}

macro_rules! index_mask {
    ($size:expr) => { ($size - 1) };
}

impl<const N: usize> Default for RingArrayBuf<N> {
    /// The ring buffer is filled with zeroes to match [`Zeroable::zeroed()`].
    fn default() -> Self {
        assert!(N.is_power_of_two(), "invalid RingArrayBuf size: should be a power of two!");
        let buffer = [0u8; N];
        RingArrayBuf { buffer, cursor: 0 }
    }
}

impl<const N: usize> Index<usize> for RingArrayBuf<N> {
    type Output = u8;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        self.buffer.index(index & index_mask!(N))
    }
}

impl<const N: usize> RingBuffer for RingArrayBuf<N> {
    const BUFFER_SIZE: usize = N;

    fn initialize(&mut self, byte: u8) {
        assert!(N.is_power_of_two(), "invalid RingArrayBuf size: should be a power of two!");
        self.buffer.fill(byte);
    }

    fn initialize_with(&mut self, init: impl FnOnce(&mut [u8])) {
        assert!(N.is_power_of_two(), "invalid RingArrayBuf size: should be a power of two!");
        init(&mut self.buffer);
    }

    #[inline(always)]
    fn cursor(&self) -> usize {
        self.cursor & index_mask!(N)
    }

    #[inline]
    fn set_cursor(&mut self, pos: isize) {
        self.cursor = pos as usize;
    }

    #[inline]
    fn push(&mut self, byte: u8) {
        assert!(N.is_power_of_two(), "invalid RingArrayBuf size: should be a power of two!");
        let cursor = self.cursor;
        self.buffer[cursor & index_mask!(N)] = byte;
        self.cursor = cursor.wrapping_add(1);
    }

    #[inline]
    fn iter_from_offset(&mut self, offset: usize) -> HistoryIter<'_, Self> {
        let offset = offset.wrapping_add(1);
        let index = self.cursor.wrapping_sub(offset);
        HistoryIter { index, ringbuf: self }
    }

    #[inline]
    fn iter_from_pos(&mut self, pos: usize) -> HistoryIter<'_, Self> {
        HistoryIter { index: pos, ringbuf: self }
    }
}

impl<'a, T: RingBuffer> Iterator for HistoryIter<'a, T> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        let res = self.ringbuf[index];
        self.index = index.wrapping_add(1);
        self.ringbuf.push(res);
        Some(res)
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use super::*;

    type TestRingBuffer = RingArrayBuf<32>;

    #[test]
    fn ringbuf_works() {
        let mut buffer = TestRingBuffer::zeroed();
        assert_eq!(buffer.cursor(), 0);
        for i in 0..32 {
            assert_eq!(buffer[i], 0);
        }
        buffer.set_cursor(-1);
        assert_eq!(buffer.cursor(), 31);
        buffer.set_cursor(100);
        assert_eq!(buffer.cursor(), 4);
        buffer.initialize(b' ');
        assert_eq!(buffer.cursor(), 4);
        for i in 0..32 {
            assert_eq!(buffer[i], b' ');
        }

        let mut buffer = bytemuck::allocation::zeroed_box::<TestRingBuffer>();
        assert_eq!(buffer.cursor(), 0);
        for i in 0..32 {
            assert_eq!(buffer[i], 0);
        }
        buffer.set_cursor(100);
        assert_eq!(buffer.cursor(), 4);
        buffer.initialize_with(|buf| {
            for (p, i) in buf.iter_mut().zip(32u8..) {
                *p = i;
            }
        });
        assert_eq!(buffer.cursor(), 4);
        for i in 0..32 {
            assert_eq!(buffer[i], (i as u8) + 32);
        }

        let mut buffer = TestRingBuffer::default();
        assert_eq!(buffer.cursor(), 0);
        for i in 0..32 {
            assert_eq!(buffer[i], 0);
        }
        buffer.initialize(b' ');
        assert_eq!(buffer.cursor(), 0);
        for i in 0..32 {
            assert_eq!(buffer[i], b' ');
        }
        buffer.push(b'!');
        assert_eq!(buffer.cursor(), 1);
        assert_eq!(buffer[0], b'!');
        assert_eq!(buffer[1], b' ');
        for i in 0..32 {
            buffer.push(i as u8);
            assert_eq!(buffer.cursor(), (i + 2) % 32);
            assert_eq!(buffer[(i + 1) % 32], i as u8);
        }
        for i in 0..32 {
            let mut buffer = TestRingBuffer::default();
            for _ in 0..i {
                buffer.push(b'.');
            }
            assert_eq!(buffer.cursor(), i);
            for n in 0..32 {
                buffer.push(n);
            }
            assert_eq!(buffer.cursor(), i);
            for _ in 0..2 {
                let vec: Vec<_> = buffer.iter_from_offset(31).take(32).collect();
                assert_eq!(buffer.cursor(), i);
                assert_eq!(vec, (0..32).collect::<Vec<u8>>());
            }
            let mut hist = buffer.iter_from_offset(15);
            let mut vec = Vec::new();
            vec.extend(hist.by_ref().take(5));
            assert_eq!(vec, (16..16+5).collect::<Vec<u8>>());
            let mut hist = buffer.iter_from_offset(15).take(11);
            vec.extend(hist.by_ref());
            assert_eq!(vec, (16..32).collect::<Vec<u8>>());
        }
    }
}
