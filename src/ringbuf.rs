//! # Ring buffer tools.
use core::fmt;
use core::mem;
use core::ops::Index;

/// A ring buffer trait.
pub trait RingBuffer: Default + Index<usize, Output=u8> {
    /// The size of the buffer in bytes.
    const BUFFER_SIZE: usize;
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
    fn iter_from_offset<'a>(&'a mut self, offset: usize) -> HistoryIter<'a, Self>;
    /// Returns an iterator which will yield consecutive bytes from the buffer starting at `pos`.
    ///
    /// At each iteration the yielded value is also being pushed to the ring buffer.
    fn iter_from_pos<'a>(&'a mut self, pos: usize) -> HistoryIter<'a, Self>;
}

/// A generic ring buffer implementation using arrays of the size of the power of two as internal buffers.
#[derive(Clone)]
pub struct RingArrayBuf<T: Copy> {
    buffer: T,
    cursor: usize
}

impl<T: Copy> fmt::Debug for RingArrayBuf<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RingArrayBuf")
         .field("buffer", &mem::size_of::<T>())
         .field("cursor", &self.cursor)
         .finish()
    }
}

/// The ring buffer history iterator.
pub struct HistoryIter<'a, T> {
    index: usize,
    ringbuf: &'a mut T
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

macro_rules! buffer_size {
    ($bits:expr) => { 1 << $bits };
}

macro_rules! index_mask {
    ($bits:expr) => { (1 << $bits) - 1 };
}

macro_rules! buffer_type {
    ($bits:expr) => { [u8; buffer_size!($bits)] };
}

macro_rules! impl_ring_buffer {
    ($bitsize:literal) => {
        impl Default for RingArrayBuf<buffer_type!($bitsize)> {
            fn default() -> Self {
                let buffer = [b' '; buffer_size!($bitsize)];
                RingArrayBuf { buffer, cursor: 0 }
            }
        }

        impl Index<usize> for RingArrayBuf<buffer_type!($bitsize)> {
            type Output = u8;

            #[inline(always)]
            fn index(&self, index: usize) -> &Self::Output {
                self.buffer.index(index & index_mask!($bitsize))
            }
        }

        impl RingBuffer for RingArrayBuf<buffer_type!($bitsize)> {
            const BUFFER_SIZE: usize = buffer_size!($bitsize);

            #[inline(always)]
            fn cursor(&self) -> usize {
                self.cursor
            }

            fn set_cursor(&mut self, pos: isize) {
                self.cursor = pos as usize & index_mask!($bitsize);
            }

            fn push(&mut self, byte: u8) {
                let index = self.cursor;
                self.buffer[index & index_mask!($bitsize)] = byte;
                self.cursor = (index + 1) & index_mask!($bitsize);
            }

            fn iter_from_offset<'a>(&'a mut self, offset: usize) -> HistoryIter<'a, Self> {
                let offset = (offset & index_mask!($bitsize)) + 1;
                let index = self.cursor + buffer_size!($bitsize) - offset;
                HistoryIter { index, ringbuf: self }
            }

            fn iter_from_pos<'a>(&'a mut self, pos: usize) -> HistoryIter<'a, Self> {
                let index = pos & index_mask!($bitsize);
                HistoryIter { index, ringbuf: self }
            }
        }
    };
}

impl_ring_buffer!(11);
impl_ring_buffer!(12);
impl_ring_buffer!(13);
impl_ring_buffer!(16);
#[cfg(feature = "lhx")]
impl_ring_buffer!(19);


#[cfg(test)]
mod tests {
    use super::*;
    impl_ring_buffer!(5);

    type TestRingBuffer = RingArrayBuf<[u8;32]>;

    #[test]
    fn ringbuf_works() {
        let mut buffer = TestRingBuffer::default();
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
