use amq_protocol::frame::parsing::traits::*;
use std::{
    iter::{Chain, Copied, Enumerate},
    slice::Iter,
};

#[derive(Debug, PartialEq)]
pub(crate) struct ParsingContext<'a> {
    buffers: [&'a [u8]; 2],
}

impl<'a> From<&'a [u8]> for ParsingContext<'a> {
    fn from(buffer: &'a [u8]) -> Self {
        Self {
            buffers: [buffer, &buffer[buffer.len()..]],
        }
    }
}

impl<'a> From<[&'a [u8]; 2]> for ParsingContext<'a> {
    fn from(buffers: [&'a [u8]; 2]) -> Self {
        Self { buffers }
    }
}

impl Clone for ParsingContext<'_> {
    fn clone(&self) -> Self {
        [self.buffers[0], self.buffers[1]].into()
    }
}

impl<'a> ParsingContext<'a> {
    pub(crate) fn as_ptr(&self) -> *const u8 {
        self.buffers[0].as_ptr()
    }

    fn iter(&self) -> Chain<Iter<'a, u8>, Iter<'a, u8>> {
        self.buffers[0].iter().chain(self.buffers[1].iter())
    }
}

impl<'a> Input for ParsingContext<'a> {
    type Item = u8;
    type Iter = Copied<Chain<Iter<'a, Self::Item>, Iter<'a, Self::Item>>>;
    type IterIndices = Enumerate<Self::Iter>;

    #[inline]
    fn input_len(&self) -> usize {
        self.buffers.iter().map(|buf| buf.len()).sum()
    }

    #[inline]
    fn take(&self, count: usize) -> Self {
        if self.buffers[0].len() > count {
            self.buffers[0][..count].into()
        } else {
            let needed = count - self.buffers[0].len();
            [self.buffers[0], &self.buffers[1][..needed]].into()
        }
    }

    #[inline]
    fn take_from(&self, index: usize) -> Self {
        if self.buffers[0].len() > index {
            [&self.buffers[0][index..], self.buffers[1]].into()
        } else {
            let needed = index - self.buffers[0].len();
            self.buffers[1][needed..].into()
        }
    }

    #[inline]
    fn take_split(&self, index: usize) -> (Self, Self) {
        if self.buffers[0].len() > index {
            (
                [&self.buffers[0][index..], self.buffers[1]].into(),
                self.buffers[0][..index].into(),
            )
        } else {
            let needed = index - self.buffers[0].len();
            (
                self.buffers[1][needed..].into(),
                [self.buffers[0], &self.buffers[1][..needed]].into(),
            )
        }
    }

    #[inline]
    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        self.iter().position(|b| predicate(*b))
    }

    #[inline]
    fn iter_elements(&self) -> Self::Iter {
        self.iter().copied()
    }

    #[inline]
    fn iter_indices(&self) -> Self::IterIndices {
        self.iter_elements().enumerate()
    }

    #[inline]
    fn slice_index(&self, count: usize) -> Result<usize, Needed> {
        if self.input_len() >= count {
            Ok(count)
        } else {
            Err(Needed::new(count - self.input_len()))
        }
    }
}

impl<'a> Compare<&'a [u8]> for ParsingContext<'_> {
    #[inline]
    fn compare(&self, buf: &'a [u8]) -> CompareResult {
        if self.iter().zip(buf).any(|(a, b)| a != b) {
            CompareResult::Error
        } else if self.input_len() >= buf.len() {
            CompareResult::Ok
        } else {
            CompareResult::Incomplete
        }
    }

    #[inline]
    fn compare_no_case(&self, buf: &'a [u8]) -> CompareResult {
        if self
            .iter()
            .zip(buf)
            .any(|(a, b)| !a.eq_ignore_ascii_case(b))
        {
            CompareResult::Error
        } else if self.input_len() >= buf.len() {
            CompareResult::Ok
        } else {
            CompareResult::Incomplete
        }
    }
}
