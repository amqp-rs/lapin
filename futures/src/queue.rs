#[derive(Debug, Clone)]
pub struct Queue {
  name: String,
}

impl Queue {
  pub fn new(name: String) -> Self {
    Self {
      name
    }
  }

  pub fn name(&self) -> String {
    self.name.clone()
  }
}
