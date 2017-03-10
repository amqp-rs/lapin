use generated::basic::Deliver;
use nom::HexDisplay;

pub trait BasicConsumer: Send {
  fn start_deliver(&mut self,
    channel_id: u16,
    method:     &Deliver);

  fn receive_content(&mut self, data: &[u8]);
}

#[derive(Clone)]
pub struct LoggingConsumer {}

impl BasicConsumer for LoggingConsumer {
  fn start_deliver(&mut self,
    channel_id: u16,
    method:     &Deliver) {
    println!("channel {} starts delivering: {:?}", channel_id, method);
  }

  fn receive_content(&mut self, data: &[u8]) {
    println!("consumer got data:\n{}", data.to_hex(16));
  }
}
