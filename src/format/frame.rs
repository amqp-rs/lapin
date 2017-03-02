use rusticata_macros::*;

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
