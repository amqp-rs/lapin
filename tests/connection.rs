extern crate lapin;
#[macro_use] extern crate nom;

use std::net::TcpStream;
use std::iter::repeat;
use std::io::{Read,Write,Error};
use std::collections::HashMap;

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

      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());

      //now connected

      //send channel
      conn.channel_open(1, "".to_string()).expect("channel_open");
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());

      //receive channel
      conn.channel_open(2, "".to_string()).expect("channel_open");
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());

      //create the hello queue
      conn.queue_declare(1, 0, "hello".to_string(), false, false, false, false, false, HashMap::new()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());

      conn.queue_declare(2, 0, "hello".to_string(), false, false, false, false, false, HashMap::new()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());
      panic!();

      println!("will consume");
      conn.basic_consume(2, 0, "hello".to_string(), "".to_string(), false, true, false, false, HashMap::new()).expect("basic_consume");
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());

      println!("will publish");
      conn.basic_publish(1, 0, "".to_string(), "hello".to_string(), false, false).expect("basic_publish");
      let payload = b"Hello world!";
      conn.send_content_frames(1, 60, payload);
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());
      panic!();
}
