extern crate lapin;
#[macro_use] extern crate nom;

use std::net::TcpStream;
use std::iter::repeat;
use std::io::{Read,Write,Error};

use nom::HexDisplay;

use lapin::connection::*;
use lapin::method::*;
use lapin::frame::*;

#[test]
fn connection() {
      let mut stream = TcpStream::connect("127.0.0.1:5672").unwrap();

      let capacity = 4096;
      let mut send_buffer: Vec<u8> = Vec::with_capacity(capacity);
      send_buffer.extend(repeat(0).take(capacity));
      let mut receive_buffer: Vec<u8> = Vec::with_capacity(capacity);
      receive_buffer.extend(repeat(0).take(capacity));

      //let (sl, bytes_written) = gen_protocol_header((&mut send_buffer, 0)).unwrap();
      //assert_eq!(stream.write(&sl[..bytes_written]).unwrap(), bytes_written);

      let mut conn: Connection = Connection::new();
      assert_eq!(conn.connect(&mut stream).unwrap(), ConnectionState::Connecting(ConnectingState::SentProtocolHeader));

      /*
      let bytes_read = stream.read(&mut receive_buffer).unwrap();

      println!("received:\n{}", (&receive_buffer[..bytes_read]).to_hex(16));
      //assert_eq!(&receive_buffer[..8], &['A' as u8, 'M' as u8, 'Q' as u8, 'P' as u8, 0, 0, 9, 1]);
      let res = frame(&receive_buffer[..bytes_read]);
      println!("received: {:?}", res);

      let payload = &receive_buffer[7..bytes_read];
      println!("payload:\n{}", (&receive_buffer[7..bytes_read]).to_hex(16));

      let res2 = method(payload);
      println!("method: {:?}", res2);
      */
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());
      panic!();
}
