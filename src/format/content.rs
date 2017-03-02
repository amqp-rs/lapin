use rusticata_macros::*;
use nom::{be_u8,be_u16,be_u64};

// 0 2 4 12 14
// +----------+--------+-----------+----------------+------------- - -
// | class-id | weight | body size | property flags | property list...
// +----------+--------+-----------+----------------+------------- - -
//    short     short    long long       short       remainder...

pub struct ContentHeader {
  class_id:       u16,
  weight:         u16,
  body_size:      u64,
  property_flags: u16,
}

named!(pub method_header<ContentHeader>,
  do_parse!(
    class:  be_u16 >>
    weight: be_u16 >>
    size:   be_u64 >>
    flags: be_u16  >>
    (ContentHeader {
      class_id:       class,
      weight:         weight,
      body_size:      size,
      property_flags: flags,
    })
  )
);


