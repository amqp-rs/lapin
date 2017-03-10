use generated::basic::Deliver;
use nom::HexDisplay;

pub trait BasicConsumer: Send {
  fn start_deliver(&mut self,
    channel_id: u16,
    method:     &Deliver);

  fn receive_content(&mut self, data: &[u8]);

  fn done(&mut self);
}

#[derive(Clone)]
pub struct LoggingConsumer {
}

impl BasicConsumer for LoggingConsumer {
  fn start_deliver(&mut self,
    channel_id: u16,
    method:     &Deliver) {
    println!("LOG channel {} starts delivering: {:?}", channel_id, method);
  }

  fn receive_content(&mut self, data: &[u8]) {
    println!("LOG consumer got data:\n{}", data.to_hex(16));
  }

  fn done(&mut self) {
    println!("LOG end of delivery");
  }
}
