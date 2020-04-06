use amq_protocol::frame::{BackToTheBuffer, GenError, GenResult, WriteContext};
use std::{cmp, io};

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Buffer {
    memory: Vec<u8>,
    capacity: usize,
    position: usize,
    end: usize,
}

pub(crate) struct Checkpoint(usize);

impl Buffer {
    pub(crate) fn with_capacity(capacity: usize) -> Buffer {
        Buffer {
            memory: vec![0; capacity],
            capacity,
            position: 0,
            end: 0,
        }
    }

    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint(self.end)
    }

    pub(crate) fn rollback(&mut self, checkpoint: Checkpoint) {
        self.end = checkpoint.0;
    }

    pub(crate) fn grow(&mut self, new_size: usize) -> bool {
        if self.capacity >= new_size {
            return false;
        }

        self.memory.resize(new_size, 0);
        self.capacity = new_size;
        true
    }

    pub(crate) fn available_data(&self) -> usize {
        self.end - self.position
    }

    fn available_space(&self) -> usize {
        self.capacity - self.end
    }

    pub(crate) fn consume(&mut self, count: usize) -> usize {
        let cnt = cmp::min(count, self.available_data());
        self.position += cnt;
        cnt
    }

    pub(crate) fn fill(&mut self, count: usize) -> usize {
        let cnt = cmp::min(count, self.available_space());
        self.end += cnt;
        cnt
    }

    pub(crate) fn write_to<T: io::Write>(&self, writer: &mut T) -> io::Result<usize> {
        writer.write(&self.memory[self.position..self.end])
    }

    pub(crate) fn read_from<T: io::Read>(&mut self, reader: &mut T) -> io::Result<usize> {
        reader.read(&mut self.memory[self.end..])
    }

    pub(crate) fn data(&self) -> &[u8] {
        &self.memory[self.position..self.end]
    }

    pub(crate) fn shift(&mut self) {
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
        if self.available_space() < size {
            self.shift();
        }
    }
}

impl io::Write for &mut Buffer {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let mut space = &mut self.memory[self.end..];
        let amt = space.write(data)?;
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
        gen: &Gen,
        before: &Before,
    ) -> Result<WriteContext<Self>, GenError> {
        if s.write.available_space() < reserved {
            return Err(GenError::BufferTooSmall(
                reserved - s.write.available_space(),
            ));
        }
        let start = s.write.checkpoint();
        s.write.fill(reserved);
        gen(s).and_then(|(s, tmp)| {
            let end = s.write.checkpoint();
            s.write.rollback(start);
            before(s, tmp).and_then(|s| {
                s.write.rollback(end);
                Ok(s)
            })
        })
    }
}
