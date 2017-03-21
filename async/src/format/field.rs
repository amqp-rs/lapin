pub fn make_bit_field(data: &[bool]) -> u8 {
  let mut res: u8 = 0;
  for (i, &val) in data.iter().enumerate() {
    if val {
      res += 1 << i;
    }
  }

  res
}

#[test]
fn bitfield() {
  let d = vec![false, true, true];
  let res:u8 = 0b00000110;
  assert_eq!(make_bit_field(&d), res);
}
