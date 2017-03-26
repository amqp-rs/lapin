extern crate lapin_async as lapin;
extern crate amq_protocol;

use std::net::TcpStream;
use std::{thread,time};

use amq_protocol::types::*;

use lapin::connection::*;
use lapin::buffer::Buffer;
use lapin::generated::basic;

#[test]
fn connection() {
      let mut stream = TcpStream::connect("127.0.0.1:5672").unwrap();
      stream.set_nonblocking(true);

      let capacity = 8192;
      let mut send_buffer    = Buffer::with_capacity(capacity as usize);
      let mut receive_buffer = Buffer::with_capacity(capacity as usize);

      let mut conn: Connection = Connection::new();
      assert_eq!(conn.connect().unwrap(), ConnectionState::Connecting(ConnectingState::SentProtocolHeader));
      loop {
        match conn.run(&mut stream, &mut send_buffer, &mut receive_buffer) {
          Err(e) => panic!("could not connect: {:?}", e),
          Ok(ConnectionState::Connected) => break,
          Ok(state) => println!("now at state {:?}, continue", state),
        }
        thread::sleep(time::Duration::from_millis(100));
      }
      println!("CONNECTED");

      //now connected

      let channel_a: u16 = conn.create_channel();
      let channel_b: u16 = conn.create_channel();
      //send channel
      conn.channel_open(channel_a, "".to_string()).expect("channel_open");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //receive channel
      conn.channel_open(channel_b, "".to_string()).expect("channel_open");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //create the hello queue
      conn.queue_declare(channel_a, 0, "hello-async".to_string(), false, false, false, false, false, FieldTable::new()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      conn.queue_declare(channel_b, 0, "hello-async".to_string(), false, false, false, false, false, FieldTable::new()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      println!("will consume");
      conn.basic_consume(channel_b, 0, "hello-async".to_string(), "my_consumer".to_string(), false, true, false, false, FieldTable::new()).expect("basic_consume");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      println!("will publish");
      conn.basic_publish(channel_a, 0, "".to_string(), "hello-async".to_string(), false, false).expect("basic_publish");
      let payload = b"Hello world!";
      conn.send_content_frames(channel_a, 60, payload, Some(basic::Properties::new()));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      let msg = conn.next_message(channel_b, "hello-async", "my_consumer").unwrap();
      println!("received message: {:?}", msg);
      println!("data: {}", std::str::from_utf8(&msg.data).unwrap());

      assert_eq!(msg.data, b"Hello world!");
}
