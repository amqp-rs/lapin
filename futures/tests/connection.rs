extern crate lapin_futures as lapin;
extern crate futures;
#[macro_use] extern crate nom;

use std::net::TcpStream;
use std::iter::repeat;
use std::io::{Read,Write,Error};
use std::collections::HashMap;
use std::{thread,time};

use nom::HexDisplay;
use lapin::*;
use lapin::client::Client;
use futures::future::{self,Future};

#[test]
fn connection() {
      let mut stream = TcpStream::connect("127.0.0.1:5672").unwrap();

      /*
      let mut client_future = Client::new(stream);
      let mut client = client_future.wait().unwrap();
      println!("got client with connection: {:?}", client.connection);
      panic!();
      */
      /*
      let i = client_future.then(|client| {
        println!("got client");
        future::ok(1)
      }).wait();
      */
      /*
      let capacity = 4096;

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

      let channel_a: u16 = conn.create_channel();
      let channel_b: u16 = conn.create_channel();
      //send channel
      conn.channel_open(channel_a, "".to_string()).expect("channel_open");
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());

      //receive channel
      conn.channel_open(channel_b, "".to_string()).expect("channel_open");
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());

      //create the hello queue
      conn.queue_declare(channel_a, 0, "hello".to_string(), false, false, false, false, false, HashMap::new()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());

      conn.queue_declare(channel_b, 0, "hello".to_string(), false, false, false, false, false, HashMap::new()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());

      println!("will consume");
      let consumer = LoggingConsumer{};
      conn.basic_consume(consumer, channel_b, 0, "hello".to_string(), "".to_string(), false, true, false, false, HashMap::new()).expect("basic_consume");
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());

      println!("will publish");
      conn.basic_publish(channel_a, 0, "".to_string(), "hello".to_string(), false, false).expect("basic_publish");
      let payload = b"Hello world!";
      conn.send_content_frames(channel_a, 60, payload);
      println!("[{}] state: {:?}", line!(), conn.write(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.read(&mut stream).unwrap());
      println!("[{}] state: {:?}", line!(), conn.parse().unwrap());
      println!("[{}] state: {:?}", line!(), conn.parse().unwrap());
      panic!();
      */
}
