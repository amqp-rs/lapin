use nom::{be_u16,be_u64};

use generated::basic::{Properties, properties};

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
  pub properties:     Properties,
}

named!(pub content_header<ContentHeader>,
  do_parse!(
    class:      be_u16     >>
    weight:     be_u16     >>
    size:       be_u64     >>
    properties: properties >>
    (ContentHeader {
      class_id:   class,
      weight:     weight,
      body_size:  size,
      properties: properties
    })
  )
);


