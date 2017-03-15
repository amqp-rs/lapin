extern crate lapin_futures as lapin;
extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate env_logger;
#[macro_use] extern crate nom;

use std::net::TcpStream;
use std::iter::repeat;
use std::io::{Read,Write,Error};
use std::collections::HashMap;
use std::{thread,time};
use std::net::SocketAddr;

use nom::HexDisplay;
use lapin::*;
//use lapin::client::Client;
use futures::future::{self,Future};
use futures::Stream;
use tokio_core::reactor::{Core,Handle};
use tokio_proto::TcpClient;

fn main() {
      env_logger::init().unwrap();
      //let mut stream = TcpStream::connect("127.0.0.1:5672").unwrap();
      let mut core = Core::new().unwrap();

      let handle = core.handle();
      let addr = "127.0.0.1:5672".parse().unwrap();

      core.run(
        lapin::client::Client::connect(&addr, &handle)
            .and_then(|client| {
//              thread::sleep_ms(3000);
              println!("client exists");
              client.create_channel().and_then(|channel| {
                let id = channel.id;
                println!("created channel with id: {}", id);
                channel.queue_declare("hello").and_then(move |_| {
                  println!("channel {} declared queue {}", id, "hello");
                  channel.basic_publish("hello", b"hello from tokio")
                })
              }).and_then(move |_| {
                client.create_channel().and_then(|channel| {
                  let id = channel.id;
                  println!("created channel with id: {}", id);
                  channel.queue_declare("hello").and_then(move |_| {
                    println!("channel {} declared queue {}", id, "hello");
                    channel.basic_consume("hello", "my_consumer").map(|stream| {
                      println!("got consumer stream");
                      stream.map(|message| {
                        println!("got message: {:?}", message);
                      })
                    })
                  })
                })
              })
              //client.ping()
              //panic!();
              //future::ok(1)
            })
            //}).map_err(|e| println!("got error: {:?}", e))
    ).unwrap();
    panic!();
}
