use amq_protocol::protocol::{constants, metadata};
use cookie_factory::*;
use nom::{be_u8,be_u16,be_u32,IResult};
use format::content::*;
use generated::*;
use generated::basic::{self, gen_properties};

named!(pub protocol_header<&[u8]>,
  preceded!(
    tag!(metadata::NAME.as_bytes()),
    tag!(&[0, metadata::MAJOR_VERSION, metadata::MINOR_VERSION, metadata::REVISION])
  )
);

pub fn gen_protocol_header<'a>(x:(&'a mut [u8],usize)) -> Result<(&'a mut [u8],usize),GenError> {
  do_gen!(
    x,
    gen_slice!(metadata::NAME.as_bytes()) >>
    gen_slice!(&[0, metadata::MAJOR_VERSION, metadata::MINOR_VERSION, metadata::REVISION])
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
    constants::FRAME_METHOD    => value!(FrameType::Method)
  | constants::FRAME_HEADER    => value!(FrameType::Header)
  | constants::FRAME_BODY      => value!(FrameType::Body)
  | constants::FRAME_HEARTBEAT => value!(FrameType::Heartbeat)
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
             tag!(&[constants::FRAME_END]) >>
    (RawFrame {
      frame_type: frame,
      channel_id: channel,
      size:       size,
      payload:    payload,
    })
  )
);

#[derive(Clone,Debug,PartialEq)]
pub enum Frame {
  ProtocolHeader,
  Method(u16, Class),
  //content header
  Header(u16, u16, ContentHeader),
  //content Body
  Body(u16, Vec<u8>),
  Heartbeat(u16)
}

pub fn frame(input: &[u8]) -> IResult<&[u8], Frame> {
  let (remaining, raw) = try_parse!(input, raw_frame);
  match raw.frame_type {
    FrameType::Header    => {
       let (_, h) = try_parse!(raw.payload, content_header);
      IResult::Done(remaining, Frame::Header(raw.channel_id, h.class_id, h))
    },
    FrameType::Body      => IResult::Done(remaining, Frame::Body(raw.channel_id, Vec::from(raw.payload))),
    FrameType::Heartbeat => IResult::Done(remaining, Frame::Heartbeat(raw.channel_id)),
    FrameType::Method    => {
      let (_, m) = try_parse!(raw.payload, parse_class);
      IResult::Done(remaining, Frame::Method(raw.channel_id, m))
    },
  }
}

pub fn gen_method_frame<'a>(input:(&'a mut [u8],usize), channel: u16, class: &Class) -> Result<(&'a mut [u8],usize),GenError> {
  //FIXME: this does not take into account the BufferTooSmall errors
  let r = gen_be_u8!(input, constants::FRAME_METHOD);
  //println!("r: {:?}", r);
  if let Ok(input1) = r {
    if let Ok((sl2, index2)) = gen_be_u16!(input1, channel) {
      if let Ok((sl3, index3)) = gen_class((sl2, index2 + 4), class) {
        if let Ok((sl4, _)) = gen_be_u32!((sl3, index2), index3 - index2 - 4) {
          gen_be_u8!((sl4, index3), constants::FRAME_END)
          //if let Ok((sl5, index5)) = gen_be_u8!((sl4, index3), constants::FRAME_END)
          //{
          //}
        } else {
          Err(GenError::CustomError(1))
        }
      } else {
        Err(GenError::CustomError(2))
      }
    } else {
      Err(GenError::CustomError(3))
    }
  } else {
    Err(GenError::CustomError(4))
  }
}

pub fn gen_heartbeat_frame<'a>(input:(&'a mut [u8],usize)) -> Result<(&'a mut [u8],usize),GenError> {
  do_gen!(input, gen_slice!(&[constants::FRAME_HEARTBEAT, 0, 0, constants::FRAME_END]))
}

pub fn gen_content_header_frame<'a>(input:(&'a mut [u8],usize), channel_id: u16, class_id: u16, length: u64, properties: &basic::Properties) -> Result<(&'a mut [u8],usize),GenError> {
  do_gen!(input,
    gen_be_u8!( constants::FRAME_HEADER ) >>
    gen_be_u16!(channel_id) >>

    ofs_len: gen_skip!(4) >>

    start: do_gen!(
      gen_be_u16!(class_id) >>
      gen_be_u16!(0) >> // weight
      gen_be_u64!(length) >>
      gen_properties(properties)
    ) >>
    end: gen_at_offset!(ofs_len, gen_be_u32!(end-start)) >>

    gen_be_u8!(constants::FRAME_END)
  )
}

pub fn gen_content_body_frame<'a>(input:(&'a mut [u8],usize), channel_id: u16, slice: &[u8]) -> Result<(&'a mut [u8],usize),GenError> {
  do_gen!(input,
    gen_be_u8!( constants::FRAME_BODY ) >>
    gen_be_u16!(channel_id) >>
    gen_be_u32!(slice.len()) >>

    gen_slice!(slice) >>

    gen_be_u8!(constants::FRAME_END)
  )
}
