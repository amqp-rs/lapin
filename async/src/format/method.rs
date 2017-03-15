use amq_protocol_types::AMQPValue;
use nom::be_u16;

use field::value;
// 0          2           4
// +----------+-----------+-------------- - -
// | class-id | method-id | arguments...
// +----------+-----------+-------------- - -
//     short      short ...

#[derive(Clone,Debug,PartialEq)]
pub struct Method {
  class_id:  u16,
  method_id: u16,
  arguments: Vec<AMQPValue>,
}

named!(pub method<Method>,
  do_parse!(
    class:  be_u16           >>
    method: be_u16           >>
    arguments: many0!(dbg_dmp!(value)) >>
    (Method {
      class_id:  class,
      method_id: method,
      arguments: arguments,
    })
  )
);


