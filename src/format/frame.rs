use rusticata_macros::*;
use nom::{be_u8,be_u16,be_u32,IResult};
use method::{method,Method};
use format::field::*;
use generated::*;

use std::collections::HashMap;

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
  pub frame_type: FrameType,
  pub channel_id: u16,
  pub size:       u32,
  pub payload:    &'a[u8],
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

pub fn gen_method_frame<'a>(input:(&'a mut [u8],usize), channel: u16, class: &Class) -> Result<(&'a mut [u8],usize),GenError> {
  if let Ok(input1) = gen_be_u8!(input, 1u8) {
    if let Ok((sl2, index2)) = gen_be_u16!(input1, channel) {
      if let Ok((sl3, index3)) = gen_class((sl2, index2 + 4), class) {
        if let Ok((sl4, index4)) = gen_be_u32!((sl3, index2), index3 - index2 - 4) {
          gen_be_u8!((sl4, index3), 0xCE)
          //if let Ok((sl5, index5)) = gen_be_u8!((sl4, index3), 0xCE)
          //{
          //}
        } else {
          Err(GenError::CustomError(42))
        }
      } else {
        Err(GenError::CustomError(42))
      }
    } else {
      Err(GenError::CustomError(42))
    }
  } else {
    Err(GenError::CustomError(42))
  }
}

pub fn gen_heartbeat_frame<'a>(input:(&'a mut [u8],usize)) -> Result<(&'a mut [u8],usize),GenError> {
  do_gen!(input, gen_slice!(&[4, 0, 0, 0xCE]))
}

pub fn gen_content_header_frame<'a>(input:(&'a mut [u8],usize), channel_id: u16, class_id: u16, length: u64) -> Result<(&'a mut [u8],usize),GenError> {
  do_gen!(input,
    gen_be_u8!( 2 ) >>
    gen_be_u16!(channel_id) >>

    gen_be_u32!( 42 ) >> //calculate static size

    gen_be_u16!(class_id) >>
    gen_be_u16!(0) >> // weight
    gen_be_u64!(length) >>
    gen_be_u8!(0) >> // properties
    gen_field_table(&HashMap::new()) >>

    gen_be_u8!(0xCE)
  )
}

pub fn gen_content_body_frame<'a>(input:(&'a mut [u8],usize), channel_id: u16, slice: &[u8]) -> Result<(&'a mut [u8],usize),GenError> {
  do_gen!(input,
    gen_be_u8!( 3 ) >>
    gen_be_u16!(channel_id) >>

    gen_slice!(slice) >>

    gen_be_u8!(0xCE)
  )
}
