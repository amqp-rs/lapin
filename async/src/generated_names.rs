use parking_lot::Mutex;

use std::{
  collections::HashMap,
  sync::Arc,
};

use crate::{
  requests::RequestId,
  types::ShortString
};

#[derive(Clone, Debug, Default)]
pub struct GeneratedNames {
  names: Arc<Mutex<HashMap<RequestId, ShortString>>>,
}

impl GeneratedNames {
  pub fn register(&self, request_id: RequestId, name: ShortString) {
    self.names.lock().insert(request_id, name);
  }

  pub fn get(&self, request_id: RequestId) -> Option<ShortString> {
    self.names.lock().remove(&request_id)
  }
}
