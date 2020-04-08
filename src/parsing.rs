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
