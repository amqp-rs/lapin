use generated::basic::Deliver;
use nom::HexDisplay;

pub trait Consumer: Send {
  fn start_deliver(&mut self,
    channel_id: u16,
    method:     Deliver);

  fn receive_content(&mut self, data: &[u8]);
}

pub struct LoggingConsumer {}

impl Consumer for LoggingConsumer {
  fn start_deliver(&mut self,
    channel_id: u16,
    method:     Deliver) {
    println!("channel {} starts delivering: {:?}", channel_id, method);
  }

  fn receive_content(&mut self, data: &[u8]) {
    println!("consumer got data:\n{}", data.to_hex(16));
  }
}
