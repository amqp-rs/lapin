use lapin_async::connection::*;
use lapin_async::api::ChannelState;
use std::io::{Error,ErrorKind,Read,Result,Write};
use futures::{Async,Poll};
use futures::future::Future;
use std::rc::Rc;
use std::cell::RefCell;

pub struct InnerClient<'consumer, T:Read+Write> {
  pub connection: Connection<'consumer>,
  pub stream:     T,
}

impl<'consumer, T:Read+Write> InnerClient<'consumer, T> {
  pub fn run(&mut self) -> Result<ConnectionState> {
    self.connection.run(&mut self.stream)
  }

  pub fn connect(&mut self) {
    self.connection.connect(&mut self.stream);
  }
}

pub struct Client<'consumer, T:Read+Write> {
  pub inner: Rc<RefCell<InnerClient<'consumer, T>>>
}

impl<'consumer, T:Read+Write+'consumer> Client<'consumer, T> {
  pub fn new(stream: T) -> Box<Future<Item=Client<'consumer, T>, Error=Error>+'consumer> {
    let mut client = Client {
        inner: Rc::new(RefCell::new(InnerClient {
        connection: Connection::new(),
        stream:     stream,
      }))
    };

    let connector = Connector { client: Some(client) };
    println!("created connector");
    Box::new(connector)
  }

  pub fn create_channel(&mut self) -> Box<Future<Item=Channel<'consumer, T>, Error=Error>+'consumer> {
    Box::new(ChannelOpener::new(
      self.inner.clone()
    ))
  }
}

pub struct Connector<'consumer, T:Read+Write> {
  client: Option<Client<'consumer, T>>,
}

impl<'consumer, T:Read+Write> Future for Connector<'consumer, T> {
  type Item  = Client<'consumer, T>;
  type Error = Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let mut client = self.client.take().unwrap();

    let state = client.inner.borrow().connection.state;

    println!("POLL state = {:?}", state);
    match state {
      ConnectionState::Initial => {
        let res = {
          let mut inner = client.inner.borrow_mut();
          inner.connect();
          inner.run()
        };
        match res {
          Ok(ConnectionState::Connected) => {
            println!("returning a client");
            return Ok(Async::Ready(client));
          },
          Ok(_) => {
            self.client = Some(client);
            Ok(Async::NotReady)
          },
          Err(e) => return Err(e),
        }
      }
      ConnectionState::Connecting(_) => {
        let res = {
          let mut inner = client.inner.borrow_mut();
          inner.run()
        };
        match res {
          Ok(ConnectionState::Connected) => {
            println!("returning a client");
            return Ok(Async::Ready(client));
          },
          Ok(_) => {
            self.client = Some(client);
            Ok(Async::NotReady)
          },
          Err(e) => {return Err(e);},
        }
      }
      s => {
        Err(Error::new(ErrorKind::ConnectionAborted, format!("could not connect: state={:?}", s)))
      }
    }
  }
}

pub struct Channel<'consumer, T:Read+Write> {
  pub inner: Rc<RefCell<InnerClient<'consumer, T>>>,
  pub id:    u16,
}

pub struct ChannelOpener<'consumer, T:Read+Write> {
  channel: Option<Channel<'consumer, T>>,
}

impl<'consumer, T:Read+Write> ChannelOpener<'consumer, T> {
  pub fn new(client: Rc<RefCell<InnerClient<'consumer, T>>>) -> ChannelOpener<'consumer, T> {
    println!("creating channel opener");
    let id = {
      let mut inner = client.borrow_mut();
      let id = inner.connection.create_channel();
      //FIXME
      inner.connection.channel_open(id, "".to_string()).expect("channel_open");
      id
    };

    ChannelOpener {
      channel: Some(Channel {
                 inner: client,
                 id:    id,
               })
    }
  }
}

impl<'consumer, T:Read+Write> Future for ChannelOpener<'consumer, T> {
  type Item  = Channel<'consumer, T>;
  type Error = Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let mut channel = self.channel.take().unwrap();

    let state: ChannelState = channel.inner.borrow().connection.get_state(channel.id).unwrap();

    println!("channel opener poll: state = {:?}", state);

    match state {
      ChannelState::Initial | ChannelState::AwaitingChannelOpenOk => {
        let res = {
          let mut inner = channel.inner.borrow_mut();
          inner.run()
        };
        match res {
          Ok(ConnectionState::Connected) => {
            let channel_state = channel.inner.borrow().connection.get_state(channel.id).expect("there should be a state");
            match channel_state {
              ChannelState::Initial | ChannelState::AwaitingChannelOpenOk => {
                Ok(Async::NotReady)
              },
              ChannelState::Connected => {
                return Ok(Async::Ready(channel));
              },
              s => {
                Err(Error::new(ErrorKind::ConnectionAborted, format!("could not connect: state={:?}", s)))
              }
            }
          },
          s => {
            Err(Error::new(ErrorKind::ConnectionAborted, format!("could not connect: state={:?}", s)))
          }
        }
      },
      ChannelState::Connected => {
        return Ok(Async::Ready(channel));
      },
      s => {
        Err(Error::new(ErrorKind::ConnectionAborted, format!("could not connect: state={:?}", s)))
      }
    }
  }
}

