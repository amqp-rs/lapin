use nom::{be_u16,be_u64};
use amq_protocol::types::*;
use amq_protocol::types::parsing::*;

// 0 2 4 12 14
// +----------+--------+-----------+----------------+------------- - -
// | class-id | weight | body size | property flags | property list...
// +----------+--------+-----------+----------------+------------- - -
//    short     short    long long       short       remainder...

#[derive(Clone,Debug,PartialEq)]
pub struct ContentHeader {
  pub class_id:       u16,
  pub weight:         u16,
  pub body_size:      u64,
  pub property_flags: u16,
  pub property_list:  FieldTable,
}

named!(pub content_header<ContentHeader>,
  do_parse!(
    class:  be_u16 >>
    weight: be_u16 >>
    size:   be_u64 >>
    flags:  be_u16 >>
    list:   parse_field_table >> //ignore the property list for now
    (ContentHeader {
      class_id:       class,
      weight:         weight,
      body_size:      size,
      property_flags: flags,
      property_list:  list,
    })
  )
);


