pub trait Store: Clone+Send+Sync+Unpin+'static {
}

#[derive(Clone)]
pub struct MemoryStore {
}

impl Default for MemoryStore {
  fn default() -> Self {
    Self {}
  }
}

impl Store for MemoryStore {
}
