extern crate lapin_futures as lapin;
extern crate futures;
extern crate futures_cpupool;
extern crate tokio_core;
extern crate env_logger;

#[macro_use] extern crate nom;

//use std::net::TcpStream;
use std::iter::repeat;
use std::io::{Read,Write,Error};
use std::collections::HashMap;
use std::{thread,time};
use std::net::SocketAddr;

use nom::HexDisplay;
use lapin::*;
//use lapin::client::Client;
use futures::future::{self,Future};
use futures_cpupool::CpuPool;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;

fn main() {
  /*
  env_logger::init().unwrap();
  let mut core = Core::new().unwrap();
  //let mut stream = TcpStream::connect(&"127.0.0.1:5672".parse::<SocketAddr>().unwrap(),  &core.handle());

  let client_future = TcpStream::connect(&"127.0.0.1:5672".parse::<SocketAddr>().unwrap(),  &core.handle()).then(|stream| {
    let stream =stream.unwrap();
    Client::new(stream)
  }).and_then(|mut client| {
    println!("will create channel");
    client.create_channel().and_then(|channel| {
      println!("channel id: {}", channel.id);
      future::ok(1)
    })
  });

  let mut channel = core.run(client_future).unwrap();
  //println!("channel id: {}", channel.id);
  */
}
