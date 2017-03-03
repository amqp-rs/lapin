use rusticata_macros::*;
use nom::{be_u8,be_u16,be_u32,IResult};
use method::{method,Method};

named!(pub protocol_header<&[u8]>,
  preceded!(
    tag!(b"AMQP\0"),
    tag!(&[0, 9, 1])
  )
);

pub fn gen_protocol_header<'a>(x:(&'a mut [u8],usize)) -> Result<(&'a mut [u8],usize),GenError> {
  do_gen!(
    x,
    gen_slice!(b"AMQP") >>
    gen_slice!(&[0, 0, 9, 1])
  )
}


// 0      1         3             7                    size+7 size+8
// +------+---------+-------------+   +------------+   +-----------+
// | type | channel |    size     |   | payload    |   | frame-end |
// +------+---------+-------------+   +------------+   +-----------+
//   octet  short        long        size octets           octet

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum FrameType {
  Method,
  Header,
  Body,
  Heartbeat
}

named!(pub frame_type<FrameType>,
  switch!(be_u8,
    1 => value!(FrameType::Method)
  | 2 => value!(FrameType::Header)
  | 3 => value!(FrameType::Body)
  | 4 => value!(FrameType::Heartbeat)
  )
);

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Channel {
  Global,
  Id(u16),
}

named!(pub channel_id<Channel>,
  switch!(be_u16,
    0 => value!(Channel::Global)
  | i => value!(Channel::Id(i))
  )
);

//FIXME: maybe parse the frame header, and leave the payload for later
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct RawFrame<'a> {
  frame_type: FrameType,
  channel_id: u16,
  size:       u32,
  payload:    &'a[u8],
}

named!(pub raw_frame<RawFrame>,
  do_parse!(
    frame:   frame_type    >>
    channel: be_u16        >>
    size:    be_u32        >>
    payload: take!(size)   >>
             tag!(&[0xCE]) >>
    (RawFrame {
      frame_type: frame,
      channel_id: channel,
      size:       size,
      payload:    payload,
    })
  )
);

#[derive(Clone,Debug,PartialEq)]
pub enum Frame<'a> {
  Method(u16, Method),
  //content header
  Header(u16, ()),
  //content Body
  Body(u16, &'a[u8]),
  Heartbeat(u16)
}

pub fn frame(input: &[u8]) -> IResult<&[u8], Frame> {
  let (remaining, raw) = try_parse!(input, raw_frame);
  match raw.frame_type {
    FrameType::Header    => IResult::Done(remaining, Frame::Header(raw.channel_id, ())),
    FrameType::Body      => IResult::Done(remaining, Frame::Body(raw.channel_id, raw.payload)),
    FrameType::Heartbeat => IResult::Done(remaining, Frame::Heartbeat(raw.channel_id)),
    FrameType::Method    => {
      println!("will try to parse a method");
      let (remaining2, m) = try_parse!(raw.payload, method);
      IResult::Done(remaining2, Frame::Method(raw.channel_id, m))
    },
  }
}
