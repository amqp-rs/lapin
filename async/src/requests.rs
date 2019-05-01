use parking_lot::Mutex;

use std::{
  collections::HashMap,
  sync::Arc,
};

pub type RequestId = u64;

#[derive(Clone, Debug, Default)]
pub struct Requests {
  finished: Arc<Mutex<HashMap<RequestId, bool>>>,
}

impl Requests {
  pub fn finish(&self, request_id: RequestId, success: bool) {
    self.finished.lock().insert(request_id, success);
  }

  pub fn was_successful(&self, request_id: RequestId) -> Option<bool> {
    self.finished.lock().remove(&request_id)
  }
}
