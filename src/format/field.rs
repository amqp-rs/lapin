use nom::{be_i8, be_i16, be_i32, be_i64, be_u8, be_u16, be_u32, be_u64, float, double};
use std::collections::HashMap;

pub type UOctet    = u8;
pub type UShort    = u16;
pub type ULong     = u32;
pub type ULongLong = u64;
pub type Timestamp = u64;

named!(pub short_string<&str>,
    do_parse!(
        length: be_u8             >>
        string: take_str!(length) >>
        (string)
    )
);

named!(pub long_string<&str>,
    do_parse!(
        length: be_u32            >>
        string: take_str!(length) >>
        (string)
    )
);
//a long string is a BE u32 followed by data. Maybe handle this in the state machine?

// Field names MUST start with a letter, '$' or '#' and may continue with letters, '$' or '#', digits, or
// underlines, to a maximum length of 128 characters.
// The server SHOULD validate field names and upon receiving an invalid field name, it SHOULD
// signal a connection exception with reply code 503 (syntax error).

#[derive(Clone,Debug,PartialEq)]
pub enum Value {
  Boolean(bool),
  ShortShortInt(i8),
  ShortShortUInt(u8),
  ShortInt(i16),
  ShortUInt(u16),
  LongInt(i32),
  LongUInt(u32),
  LongLongInt(i64),
  LongLongUInt(u64),
  Float(f32),
  Double(f64),
  Decimal(f32),
  ShortString(String),
  LongString(String),
  Array(Vec<Value>),
  Timestamp(u64),
  Table(HashMap<String,Value>),
  None
}

named!(pub value<Value>,
  switch!(map!(be_u8, |u| u as char),
    't' => call!(parse_boolean)          |
    //FIXME: the spec says b for i8, B for u8, but U for i16, u for u16, I for i32, i for u32, etc
    // is that right?
    'b' => call!(parse_short_short_int)  |
    'B' => call!(parse_short_short_uint) |
    'U' => call!(parse_short_int)        |
    'u' => call!(parse_short_uint)       |
    'I' => call!(parse_long_int)         |
    'i' => call!(parse_long_uint)        |
    'L' => call!(parse_long_long_int)    |
    'l' => call!(parse_long_long_uint)   |
    'f' => call!(parse_float)            |
    'd' => call!(parse_double)           |
    'D' => call!(parse_decimal)          |
    's' => call!(parse_short_string)     |
    'S' => call!(parse_long_string)      |
    'A' => call!(parse_array)            |
    'T' => call!(parse_timestamp)        |
    'F' => call!(parse_table)            |
    'V' => value!(Value::None)
  )
);
named!(pub field_name_value<(String, Value)>,
  tuple!(map!(short_string, |s:&str| s.to_string()), value)
);

named!(parse_boolean<Value>,
  map!(be_u8, |u| Value::Boolean(u != 0))
);

named!(parse_short_short_int<Value>,
  map!(be_i8, |i| Value::ShortShortInt(i))
);
named!(parse_short_short_uint<Value>,
  map!(be_u8, |i| Value::ShortShortUInt(i))
);
named!(parse_short_int<Value>,
  map!(be_i16, |i| Value::ShortInt(i))
);
named!(parse_short_uint<Value>,
  map!(be_u16, |i| Value::ShortUInt(i))
);
named!(parse_long_int<Value>,
  map!(be_i32, |i| Value::LongInt(i))
);
named!(parse_long_uint<Value>,
  map!(be_u32, |i| Value::LongUInt(i))
);
named!(parse_long_long_int<Value>,
  map!(be_i64, |i| Value::LongLongInt(i))
);
named!(parse_long_long_uint<Value>,
  map!(be_u64, |i| Value::LongLongUInt(i))
);
named!(parse_float<Value>,
  map!(float, |i| Value::Float(i))
);
named!(parse_double<Value>,
  map!(double, |i| Value::Double(i))
);
named!(parse_decimal<Value>,
  map!(float, |i| Value::Decimal(i))
);
named!(parse_short_string<Value>,
  map!(short_string, |s:&str| Value::ShortString(s.to_string()))
);

named!(parse_long_string<Value>,
  map!(long_string, |s:&str| Value::LongString(s.to_string()))
);

named!(parse_array<Value>,
  do_parse!(
    quantity: be_u32 >>
    //FIXME: the spec specifies a long int there, but a long uint for the table?
    vec: map!(count!(value, quantity as usize), |v| Value::Array(v)) >>
    (vec)
  )
);
named!(parse_timestamp<Value>,
  map!(be_u64, |i| Value::Timestamp(i))
);
named!(parse_table<Value>,
  do_parse!(
    //FIXME: the spec specifies a long uint there, but a long int for the array?
    quantity: be_u32 >>
    h: map!(count!(field_name_value, quantity as usize), |v:Vec<(String,Value)>| {
      Value::Table(v.iter().cloned().collect())
    }) >>
    (h)
  )
);

named!(pub field_table<HashMap<String,Value>>,
  do_parse!(
    //FIXME: the spec specifies a long uint there, but a long int for the array?
    quantity: be_u32 >>
    h: map!(count!(field_name_value, quantity as usize), |v:Vec<(String,Value)>| {
      v.iter().cloned().collect()
    }) >>
    (h)
  )
);
