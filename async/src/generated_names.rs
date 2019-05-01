use parking_lot::Mutex;

use std::{
  collections::HashMap,
  sync::Arc,
};

use crate::requests::RequestId;

#[derive(Clone, Debug, Default)]
pub struct GeneratedNames {
  names: Arc<Mutex<HashMap<RequestId, String>>>,
}

impl GeneratedNames {
  pub fn register(&self, request_id: RequestId, name: String) {
    self.names.lock().insert(request_id, name);
  }

  pub fn get(&self, request_id: RequestId) -> Option<String> {
    self.names.lock().remove(&request_id)
  }
}
