use lapin_async::connection::*;
use std::io::{Error,ErrorKind,Read,Write};
use futures::{Async,Poll};
use futures::future::Future;

pub struct Client<'consumer, T:Read+Write+Send> {
  pub connection: Connection<'consumer>,
  pub stream:     T,
}

impl<'consumer, T:Read+Write+Send+'consumer> Client<'consumer, T> {
  pub fn new(stream: T) -> Box<Future<Item=Client<'consumer, T>, Error=Error>+'consumer> {
    let mut client = Client {
      connection: Connection::new(),
      stream:     stream,
    };

    let connector = Connector { client: Some(client) };
    println!("created connector");
    Box::new(connector)
  }
}

pub struct Connector<'consumer, T:Read+Write+Send> {
  client: Option<Client<'consumer, T>>,
}

impl<'consumer, T:Read+Write+Send> Future for Connector<'consumer, T> {
  type Item  = Client<'consumer, T>;
  type Error = Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let mut client = self.client.take().unwrap();

    let state = client.connection.state;

    println!("POLL state = {:?}", state);
    match state {
      ConnectionState::Initial => {
        client.connection.connect(&mut client.stream);
        match client.connection.run(&mut client.stream) {
          Ok(ConnectionState::Connected) => {
            Ok(Async::Ready(client))
          },
          Ok(_) => {
            self.client = Some(client);
            Ok(Async::NotReady)
          },
          Err(e) => return Err(e),
        }
      }
      ConnectionState::Connecting(_) => {
        match client.connection.run(&mut client.stream) {
          Ok(ConnectionState::Connected) => {
            Ok(Async::Ready(client))
          },
          Ok(_) => {
            self.client = Some(client);
            Ok(Async::NotReady)
          },
          Err(e) => Err(e),
        }
      }
      s => {
        Err(Error::new(ErrorKind::ConnectionAborted, format!("could not connect: state={:?}", s)))
      }
    }
  }

}
