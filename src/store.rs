pub trait Store: Send+Sync+'static {
}

pub struct MemoryStore {
}

impl Default for MemoryStore {
  fn default() -> Self {
    Self {}
  }
}

impl Store for MemoryStore {
}
