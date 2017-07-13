#[macro_use] extern crate log;
extern crate env_logger;
extern crate futures;
extern crate lapin_futures as lapin;
extern crate rustls;
extern crate tokio_core;
extern crate tokio_rustls;
extern crate webpki_roots;

use futures::future::Future;
use lapin::client::ConnectionOptions;
use lapin::channel::ConfirmSelectOptions;
use rustls::ClientConfig;
use std::sync::Arc;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;
use tokio_rustls::ClientConfigExt;

fn main() {
  env_logger::init().unwrap();

  let host       = "localhost";
  let port       = 5671;
  let username   = "guest";
  let password   = "guest";

  let mut config = ClientConfig::new();
  config.root_store.add_trust_anchors(&webpki_roots::ROOTS);
  let config     = Arc::new(config);
  let mut core   = Core::new().unwrap();
  let handle     = core.handle();
  let raw_stream = std::net::TcpStream::connect((host, port)).unwrap();

  core.run(
    TcpStream::from_stream(raw_stream, &handle).map(|stream| futures::future::ok(stream)).unwrap().and_then(|stream| {
      config.connect_async(host, stream)
    }).and_then(|stream| {
      lapin::client::Client::connect(stream, &ConnectionOptions {
        username: username.to_string(),
        password: password.to_string(),
        ..Default::default()
      })
    }).and_then(|(client, _)| {
      client.create_confirm_channel(ConfirmSelectOptions::default()).and_then(|channel| {
        let id = channel.id;
        info!("created channel with id: {}", id);
        Ok(())
      })
    })
  ).unwrap();
}
