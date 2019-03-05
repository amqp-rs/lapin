use std::{cmp, io::{self, Write, Read}, iter::repeat, ptr};

#[derive(Debug,PartialEq,Clone)]
pub struct Buffer {
  memory:   Vec<u8>,
  capacity: usize,
  position: usize,
  end:      usize
}

impl Buffer {
  pub fn with_capacity(capacity: usize) -> Buffer {
    let mut v = Vec::with_capacity(capacity);
    v.extend(repeat(0).take(capacity));
    Buffer {
      memory:   v,
      capacity,
      position: 0,
      end:      0
    }
  }

  pub fn grow(&mut self, new_size: usize) -> bool {
    if self.capacity >= new_size {
      return false;
    }

    self.memory.resize(new_size, 0);
    self.capacity = new_size;
    true
  }

  pub fn available_data(&self) -> usize {
    self.end - self.position
  }

  pub fn available_space(&self) -> usize {
    self.capacity - self.end
  }

  pub fn consume(&mut self, count: usize) -> usize {
    let cnt        = cmp::min(count, self.available_data());
    self.position += cnt;
    if self.position > self.capacity / 2 {
      self.shift();
    }
    cnt
  }

  pub fn fill(&mut self, count: usize) -> usize {
    if self.available_space() < self.position {
      self.shift();
    }
    let cnt   = cmp::min(count, self.available_space());
    self.end += cnt;
    cnt
  }

  pub fn data(&self) -> &[u8] {
    &self.memory[self.position..self.end]
  }

  pub fn space(&mut self) -> &mut [u8] {
    &mut self.memory[self.end..self.capacity]
  }

  fn shift(&mut self) {
    let length = self.end - self.position;
    unsafe {
      ptr::copy((&self.memory[self.position..self.end]).as_ptr(), (&mut self.memory[..length]).as_mut_ptr(), length);
    }
    self.position = 0;
    self.end      = length;
  }
}

impl Write for Buffer {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    self.space().write(buf).map(|size| {
      self.fill(size);
      size
    })
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

impl Read for Buffer {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    let len = cmp::min(self.available_data(), buf.len());
    buf.copy_from_slice(&self.memory[self.position..len]);
    self.position += len;
    Ok(len)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Write;

  #[test]
  fn fill_and_consume() {
    let mut b = Buffer::with_capacity(10);
    assert_eq!(b.available_data(), 0);
    assert_eq!(b.available_space(), 10);
    let res = b.write(&b"abcd"[..]);
    assert_eq!(res.ok(), Some(4));
    assert_eq!(b.available_data(), 4);
    assert_eq!(b.available_space(), 6);

    assert_eq!(b.data(), &b"abcd"[..]);

    b.consume(2);
    assert_eq!(b.available_data(), 2);
    assert_eq!(b.available_space(), 6);
    assert_eq!(b.data(), &b"cd"[..]);

    b.shift();
    assert_eq!(b.available_data(), 2);
    assert_eq!(b.available_space(), 8);
    assert_eq!(b.data(), &b"cd"[..]);

    assert_eq!(b.write(&b"efghijklmnop"[..]).ok(), Some(8));
    assert_eq!(b.available_data(), 10);
    assert_eq!(b.available_space(), 0);
    assert_eq!(b.data(), &b"cdefghijkl"[..]);
    b.shift();
    assert_eq!(b.available_data(), 10);
    assert_eq!(b.available_space(), 0);
    assert_eq!(b.data(), &b"cdefghijkl"[..]);
  }
}
