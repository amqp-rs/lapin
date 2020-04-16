use amq_protocol::frame::{BackToTheBuffer, GenError, GenResult, WriteContext};
use std::{
    cmp,
    io::{self, IoSlice, IoSliceMut},
};

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Buffer {
    memory: Vec<u8>,
    capacity: usize,
    position: usize,
    end: usize,
    available_data: usize,
}

// The bool is true when the checkpoint is to go backwards, false for forward
pub(crate) struct Checkpoint(usize, bool);

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
        Checkpoint(self.end, true)
    }

    pub(crate) fn rollback(&mut self, checkpoint: Checkpoint) {
        if checkpoint.1 {
            if self.end > checkpoint.0 {
                self.available_data -= self.end - checkpoint.0;
            } else {
                self.available_data -= self.end + (self.capacity - checkpoint.0);
            }
        } else {
            if self.end > checkpoint.0 {
                self.available_data += (self.capacity - self.end) + checkpoint.0;
            } else {
                self.available_data += checkpoint.0 - self.end;
            }
        }
        self.end = checkpoint.0;
    }

    pub(crate) fn grow(&mut self, new_size: usize) -> bool {
        if self.capacity >= new_size {
            return false;
        }

        let old_capacity = self.capacity;

        self.memory.resize(new_size, 0);
        self.capacity = new_size;

        if self.end < self.position {
            // FIXME: make sure there's enough room
            let (old, new) = self.memory.split_at_mut(old_capacity);
            new[..].copy_from_slice(&old[..self.end]);
            self.end += old_capacity;
        }

        true
    }

    pub(crate) fn available_data(&self) -> usize {
        self.available_data
    }

    fn available_space(&self) -> usize {
        self.capacity - self.available_data
    }

    pub(crate) fn consume(&mut self, count: usize, ring: bool) -> usize {
        let cnt = cmp::min(count, self.available_data());
        self.position += cnt;
        if ring {
            self.position %= self.capacity;
        }
        self.available_data -= cnt;
        cnt
    }

    pub(crate) fn fill(&mut self, count: usize, ring: bool) -> usize {
        let cnt = cmp::min(count, self.available_space());
        self.end += cnt;
        if ring {
            self.end %= self.capacity;
        }
        self.available_data += cnt;
        cnt
    }

    pub(crate) fn write_to<T: io::Write>(&self, writer: &mut T) -> io::Result<usize> {
        if self.available_data() == 0 {
            Ok(0)
        } else {
            if self.end > self.position {
                writer.write(&self.memory[self.position..self.end])
            } else {
                writer.write_vectored(&[
                    IoSlice::new(&self.memory[self.position..]),
                    IoSlice::new(&self.memory[..self.end]),
                ])
            }
        }
    }

    pub(crate) fn read_from<T: io::Read>(&mut self, reader: &mut T, ring: bool) -> io::Result<usize> {
        if self.available_space() == 0 {
            Ok(0)
        } else {
            if ring {
                if self.end >= self.position {
                    let (start, end) = self.memory.split_at_mut(self.end);
                    reader.read_vectored(
                        &mut [
                        IoSliceMut::new(&mut end[..]),
                        IoSliceMut::new(&mut start[..self.position]),
                        ][..],
                    )
                } else {
                    reader.read(&mut self.memory[self.end..self.position])
                }
            } else {
                reader.read(&mut self.memory[self.end..])
            }
        }
    }

    pub(crate) fn offset(&self, buf: &[u8]) -> usize {
        let bufptr = buf.as_ptr() as usize;
        if self.end >= self.position {
            let data = &self.memory[self.position..self.end];
            bufptr - data.as_ptr() as usize
        } else {
            let data = &self.memory[self.position..];
            let dataptr = data.as_ptr() as usize;
            if dataptr < bufptr {
                bufptr - dataptr
            } else {
                let data = &self.memory[..self.end];
                bufptr + self.capacity - self.position - data.as_ptr() as usize
            }
        }
    }

    pub(crate) fn data(&self) -> &[u8] {
        // FIXME: drop
        &self.memory[self.position..self.end]
    }

    pub(crate) fn shift(&mut self) {
        // FIXME: drop
        let length = self.end - self.position;
        if length > self.position {
            return;
        } else {
            let (start, end) = self.memory.split_at_mut(self.position);
            start[..length].copy_from_slice(&end[..length]);
        }
        self.position = 0;
        self.end = length;
    }

    pub(crate) fn shift_unless_available(&mut self, size: usize) {
        // FIXME: drop
        if self.available_space() < size {
            self.shift();
        }
    }
}

impl io::Write for &mut Buffer {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let amt = if self.available_space() == 0 {
            0
        } else {
            if self.end >= self.position {
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
            }
        };
        self.fill(amt, true);
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
        gen: &Gen,
        before: &Before,
    ) -> Result<WriteContext<Self>, GenError> {
        if s.write.available_space() < reserved {
            return Err(GenError::BufferTooSmall(
                reserved - s.write.available_space(),
            ));
        }
        let start = s.write.checkpoint();
        s.write.fill(reserved, true);
        gen(s).and_then(|(s, tmp)| {
            let mut end = s.write.checkpoint();
            end.1 = false;
            s.write.rollback(start);
            before(s, tmp).and_then(|s| {
                s.write.rollback(end);
                Ok(s)
            })
        })
    }
}
