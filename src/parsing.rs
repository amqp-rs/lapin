use amq_protocol::frame::parsing::traits::*;
use std::{
    iter::{Chain, Cloned, Enumerate},
    ops::RangeFrom,
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

impl<'a> Clone for ParsingContext<'a> {
    fn clone(&self) -> Self {
        [&self.buffers[0][..], &self.buffers[1][..]].into()
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

impl<'a> InputIter for ParsingContext<'a> {
    type Item = u8;
    type Iter = Enumerate<Self::IterElem>;
    type IterElem = Cloned<Chain<Iter<'a, u8>, Iter<'a, u8>>>;

    #[inline]
    fn iter_indices(&self) -> Self::Iter {
        self.iter_elements().enumerate()
    }

    #[inline]
    fn iter_elements(&self) -> Self::IterElem {
        self.iter().cloned()
    }

    #[inline]
    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        self.iter().position(|b| predicate(*b))
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

impl<'a> InputLength for ParsingContext<'a> {
    #[inline]
    fn input_len(&self) -> usize {
        self.buffers.iter().map(|buf| buf.len()).sum()
    }
}

impl<'a> InputTake for ParsingContext<'a> {
    #[inline]
    fn take(&self, count: usize) -> Self {
        if self.buffers[0].len() > count {
            self.buffers[0][..count].into()
        } else {
            let needed = count - self.buffers[0].len();
            [&self.buffers[0][..], &self.buffers[1][..needed]].into()
        }
    }

    #[inline]
    fn take_split(&self, count: usize) -> (Self, Self) {
        if self.buffers[0].len() > count {
            (
                [&self.buffers[0][count..], &self.buffers[1][..]].into(),
                self.buffers[0][..count].into(),
            )
        } else {
            let needed = count - self.buffers[0].len();
            (
                self.buffers[1][needed..].into(),
                [&self.buffers[0][..], &self.buffers[1][..needed]].into(),
            )
        }
    }
}

impl<'a> Slice<RangeFrom<usize>> for ParsingContext<'a> {
    #[inline]
    fn slice(&self, range: RangeFrom<usize>) -> Self {
        if range.start < self.buffers[0].len() {
            [&self.buffers[0][range.start..], &self.buffers[1][..]].into()
        } else {
            let needed = range.start - self.buffers[0].len();
            self.buffers[1][needed..].into()
        }
    }
}

impl<'a> UnspecializedInput for ParsingContext<'a> {}
