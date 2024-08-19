use crate::parsing::ParsingContext;
use amq_protocol::frame::{BackToTheBuffer, GenError, GenResult, WriteContext};
use futures_io::{AsyncRead, AsyncWrite};
use std::{
    cmp,
    io::{self, IoSlice, IoSliceMut},
    pin::Pin,
    task::{Context, Poll},
};

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
    backwards: bool,
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

    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            end: self.end,
            backwards: true,
        }
    }

    pub(crate) fn rollback(&mut self, checkpoint: Checkpoint) {
        if checkpoint.end == self.end {
            return;
        }
        if checkpoint.backwards {
            if self.end > checkpoint.end {
                self.available_data -= self.end - checkpoint.end;
            } else {
                self.available_data -= self.end + (self.capacity - checkpoint.end);
            }
        } else if self.end > checkpoint.end {
            self.available_data += (self.capacity - self.end) + checkpoint.end;
        } else {
            self.available_data += checkpoint.end - self.end;
        }
        self.end = checkpoint.end;
    }

    pub(crate) fn grow(&mut self, new_size: usize) -> bool {
        if self.capacity >= new_size {
            return false;
        }

        let old_capacity = self.capacity;
        let growth = new_size - old_capacity;

        self.memory.resize(new_size, 0);
        self.capacity = new_size;

        if self.end <= self.position && self.available_data > 0 {
            // We have data and the "tail" was at the beginning of the buffer.
            // We need to move it in the new end.
            let (old, new) = self.memory.split_at_mut(old_capacity);
            if self.end < growth {
                // There is enough room in the new end for this whole "tail".
                new[..].copy_from_slice(&old[..self.end]);
                self.end += old_capacity;
            } else {
                // Fill the new end with as much data as we can.
                // We also update the end pointer to the future right location.
                // We still have [growth..old_end] to move into [..new_end]
                new[..].copy_from_slice(&old[..growth]);
                self.end -= growth;
                if self.end < growth {
                    // Less than half the data is yet to be moved, we can split + copy.
                    let (start, data) = self.memory.split_at_mut(growth);
                    start[..].copy_from_slice(&data[..self.end])
                } else {
                    // Not enough room to split + copy, we copy each byte one at a time.
                    for i in 0..=self.end {
                        self.memory[i] = self.memory[i + growth];
                    }
                }
            }
        }

        true
    }

    pub(crate) fn available_data(&self) -> usize {
        self.available_data
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

    pub(crate) fn poll_write_to<T: AsyncWrite>(
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

    pub(crate) fn poll_read_from<T: AsyncRead>(
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
        let data = &self.memory[self.position..self.position];
        let dataptr = data.as_ptr() as usize;
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
            let mut end = s.write.checkpoint();
            end.backwards = false;
            s.write.rollback(start);
            before(s, tmp).map(|s| {
                s.write.rollback(end);
                s
            })
        })
    }
}
