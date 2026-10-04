use crate::parsing::ParsingContext;
use amq_protocol::frame::{BackToTheBuffer, GenError, GenResult, WriteContext};
use futures_io::{AsyncRead, AsyncWrite};
use std::{
    cmp,
    collections::TryReserveError,
    io::{self, IoSlice, IoSliceMut},
    pin::Pin,
    task::{Context, Poll},
};

/// The type, channel and size that precede a frame's payload
const FRAME_HEADER_LEN: usize = 7;
/// The bytes a frame adds to its payload: its header and its end octet
pub(crate) const FRAME_OVERHEAD: usize = FRAME_HEADER_LEN + 1;

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Buffer {
    memory: Vec<u8>,
    capacity: usize,
    position: usize,
    end: usize,
    available_data: usize,
}

pub(crate) struct Checkpoint {
    end: usize,
    available_data: usize,
}

impl Buffer {
    pub(crate) fn with_capacity(capacity: usize) -> Buffer {
        Buffer {
            memory: vec![0; capacity],
            capacity,
            position: 0,
            end: 0,
            available_data: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.position = 0;
        self.end = 0;
        self.available_data = 0;
    }

    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            end: self.end,
            available_data: self.available_data,
        }
    }

    pub(crate) fn rollback(&mut self, checkpoint: Checkpoint) {
        self.end = checkpoint.end;
        self.available_data = checkpoint.available_data;
    }

    pub(crate) fn grow(&mut self, new_size: usize) -> Result<bool, TryReserveError> {
        if self.capacity >= new_size {
            return Ok(false);
        }

        let old_capacity = self.capacity;
        let growth = new_size - old_capacity;

        self.memory.try_reserve_exact(new_size - old_capacity)?;
        self.memory.resize(new_size, 0);
        self.capacity = new_size;

        if self.end <= self.position && self.available_data > 0 {
            // We have data and the "tail" was at the beginning of the buffer.
            // We need to move it in the new end.
            let (old, new) = self.memory.split_at_mut(old_capacity);
            if self.end < growth {
                // There is enough room in the new end for this whole "tail".
                new[..self.end].copy_from_slice(&old[..self.end]);
                self.end += old_capacity;
            } else {
                // Fill the new end with as much data as we can.
                new.copy_from_slice(&old[..growth]);
                // We still have [growth..old_end] to move into [0..new_end]
                // copy_within handles the overlap correctly, if any.
                self.memory.copy_within(growth..self.end, 0);
                // We also update the end pointer to the right location.
                self.end -= growth;
            }
        }

        Ok(true)
    }

    pub(crate) fn available_data(&self) -> usize {
        self.available_data
    }

    pub(crate) fn capacity(&self) -> usize {
        self.capacity
    }

    pub(crate) fn frame_wire_len(&self) -> Option<u64> {
        if self.available_data < FRAME_HEADER_LEN {
            return None;
        }
        let byte = |offset: usize| self.memory[(self.position + offset) % self.capacity];
        if [byte(0), byte(1), byte(2), byte(3)] == *b"AMQP" {
            return None;
        }
        let size = u32::from_be_bytes([byte(3), byte(4), byte(5), byte(6)]) as u64;
        Some(size + FRAME_OVERHEAD as u64)
    }

    pub(crate) fn available_space(&self) -> usize {
        self.capacity - self.available_data
    }

    pub(crate) fn consume(&mut self, count: usize) -> usize {
        let cnt = cmp::min(count, self.available_data());
        self.position += cnt;
        self.position %= self.capacity;
        self.available_data -= cnt;
        cnt
    }

    pub(crate) fn fill(&mut self, count: usize) -> usize {
        let cnt = cmp::min(count, self.available_space());
        self.end += cnt;
        self.end %= self.capacity;
        self.available_data += cnt;
        cnt
    }

    pub(crate) fn poll_write_to<T: AsyncWrite + ?Sized>(
        &self,
        cx: &mut Context<'_>,
        writer: Pin<&mut T>,
    ) -> Poll<io::Result<usize>> {
        if self.available_data() == 0 {
            Poll::Ready(Ok(0))
        } else if self.end > self.position {
            writer.poll_write(cx, &self.memory[self.position..self.end])
        } else {
            writer.poll_write_vectored(
                cx,
                &[
                    IoSlice::new(&self.memory[self.position..]),
                    IoSlice::new(&self.memory[..self.end]),
                ],
            )
        }
    }

    pub(crate) fn poll_read_from<T: AsyncRead + ?Sized>(
        &mut self,
        cx: &mut Context<'_>,
        reader: Pin<&mut T>,
    ) -> Poll<io::Result<usize>> {
        if self.available_space() == 0 {
            Poll::Ready(Ok(0))
        } else if self.end >= self.position {
            let (start, end) = self.memory.split_at_mut(self.end);
            reader.poll_read_vectored(
                cx,
                &mut [
                    IoSliceMut::new(&mut *end),
                    IoSliceMut::new(&mut start[..self.position]),
                ][..],
            )
        } else {
            reader.poll_read(cx, &mut self.memory[self.end..self.position])
        }
    }

    pub(crate) fn offset(&self, buf: ParsingContext<'_>) -> usize {
        let dataptr = self.memory[self.position..].as_ptr() as usize;
        let bufptr = buf.as_ptr() as usize;

        if dataptr < bufptr {
            bufptr - dataptr
        } else {
            let start = &self.memory[..0];
            let startptr = start.as_ptr() as usize;
            bufptr + self.capacity - self.position - startptr
        }
    }

    pub(crate) fn parsing_context(&self) -> ParsingContext<'_> {
        if self.available_data() == 0 {
            self.memory[self.end..self.end].into()
        } else if self.end > self.position {
            self.memory[self.position..self.end].into()
        } else {
            [&self.memory[self.position..], &self.memory[..self.end]].into()
        }
    }
}

impl io::Write for &mut Buffer {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let amt = if self.available_space() == 0 {
            0
        } else if self.end >= self.position {
            let mut space = &mut self.memory[self.end..];
            let mut amt = space.write(data)?;
            if amt == self.capacity - self.end {
                let mut space = &mut self.memory[..self.position];
                amt += space.write(&data[amt..])?;
            }
            amt
        } else {
            let mut space = &mut self.memory[self.end..self.position];
            space.write(data)?
        };
        self.fill(amt);
        Ok(amt)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl BackToTheBuffer for &mut Buffer {
    fn reserve_write_use<
        Tmp,
        Gen: Fn(WriteContext<Self>) -> Result<(WriteContext<Self>, Tmp), GenError>,
        Before: Fn(WriteContext<Self>, Tmp) -> GenResult<Self>,
    >(
        s: WriteContext<Self>,
        reserved: usize,
        generator: &Gen,
        before: &Before,
    ) -> Result<WriteContext<Self>, GenError> {
        if s.write.available_space() < reserved {
            return Err(GenError::BufferTooSmall(
                reserved - s.write.available_space(),
            ));
        }
        let start = s.write.checkpoint();
        s.write.fill(reserved);
        generator(s).and_then(|(s, tmp)| {
            let end = s.write.checkpoint();
            s.write.rollback(start);
            #[allow(clippy::manual_inspect)]
            before(s, tmp).map(|s| {
                s.write.rollback(end);
                s
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Buffer;

    #[test]
    fn grow_preserves_wrapped_data_when_tail_fits() {
        let mut buffer = Buffer::with_capacity(8);
        buffer.memory[6..8].copy_from_slice(&[1, 2]);
        buffer.memory[..2].copy_from_slice(&[3, 4]);
        buffer.position = 6;
        buffer.end = 2;
        buffer.available_data = 4;

        assert!(buffer.grow(16).unwrap());
        assert_eq!(buffer.capacity, 16);
        assert_eq!(buffer.available_data(), 4);
        assert_eq!(&buffer.memory[buffer.position..buffer.end], &[1, 2, 3, 4]);
    }

    #[test]
    fn grow_preserves_wrapped_data_when_tail_exceeds_growth() {
        // A tail longer than the growth, then one exactly as long.
        for tail in [3u8, 2] {
            let mut buffer = Buffer::with_capacity(8);
            buffer.memory[5..8].copy_from_slice(&[1, 2, 3]);
            let tail_data = (10..10 + tail).collect::<Vec<_>>();
            buffer.memory[..tail as usize].copy_from_slice(&tail_data);
            buffer.position = 5;
            buffer.end = tail as usize;
            buffer.available_data = 3 + tail as usize;

            assert!(buffer.grow(10).unwrap());
            assert_eq!(buffer.capacity, 10);
            assert_eq!(buffer.available_data(), 3 + tail as usize);
            assert_eq!(
                buffer.end,
                (buffer.position + buffer.available_data()) % buffer.capacity
            );
            let data = (0..buffer.available_data())
                .map(|offset| buffer.memory[(buffer.position + offset) % buffer.capacity])
                .collect::<Vec<_>>();
            assert_eq!(data, [&[1, 2, 3][..], &tail_data].concat());
        }
    }

    #[test]
    fn rollback_restores_empty_state_after_a_full_wrap() {
        let mut buffer = Buffer::with_capacity(8);
        let checkpoint = buffer.checkpoint();
        buffer.fill(8);
        assert_eq!(buffer.end, checkpoint.end);
        buffer.rollback(checkpoint);
        assert_eq!(buffer.available_data(), 0);
        assert_eq!(buffer.available_space(), 8);
    }

    #[test]
    fn frame_size_hint_reads_wrapped_header() {
        let mut buffer = Buffer::with_capacity(10);
        buffer.position = 7;
        buffer.end = 4;
        buffer.available_data = 7;
        for (index, byte) in [1, 0, 0, 0, 0, 1, 0].iter().enumerate() {
            buffer.memory[(7 + index) % 10] = *byte;
        }
        assert_eq!(buffer.frame_wire_len(), Some(264));
    }

    #[test]
    fn frame_size_hint_does_not_overflow_on_32_bit_targets() {
        let mut buffer = Buffer::with_capacity(7);
        buffer
            .memory
            .copy_from_slice(&[1, 0, 0, 255, 255, 255, 255]);
        buffer.fill(7);
        assert_eq!(buffer.frame_wire_len(), Some(u32::MAX as u64 + 8));
    }
}
