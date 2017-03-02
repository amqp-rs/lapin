use rusticata_macros::*;
use nom::{be_u8,be_u16,be_u64};

// 0          2           4
// +----------+-----------+-------------- - -
// | class-id | method-id | arguments...
// +----------+-----------+-------------- - -
//     short      short ...

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct MethodHeader {
  class_id:  u16,
  method_id: u16,
}

named!(pub method_header<MethodHeader>,
  do_parse!(
    class:  be_u16 >>
    method: be_u16 >>
    (MethodHeader {
      class_id:  class,
      method_id: method,
    })
  )
);


