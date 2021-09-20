use crate::Error;
use sodiumoxide::crypto;
use std::convert::TryInto;
pub type Keypair = ([u8;32],[u8;64]);

#[async_trait::async_trait]
pub trait Store: Clone+Send+Sync+Unpin+'static {
  async fn get_keypair(&mut self) -> Result<Option<Keypair>,Error>;
  async fn set_keypair(&mut self, keypair: Keypair) -> Result<(),Error>;
  async fn get_or_create_keypair(&mut self) -> Result<Keypair,Error> {
    if let Some(kp) = self.get_keypair().await? {
      Ok(kp)
    } else {
      let (pk,sk) = crypto::sign::gen_keypair();
      let kp = (
        pk.as_ref().try_into().unwrap(),
        sk.as_ref().try_into().unwrap()
      );
      self.set_keypair(kp.clone()).await?;
      Ok(kp)
    }
  }
  async fn get_latest(&mut self, channel: &[u8]) -> Result<[u8;32],Error>;
}

#[derive(Clone)]
pub struct MemoryStore {
  keypair: Keypair,
}

impl Default for MemoryStore {
  fn default() -> Self {
    let (pk,sk) = crypto::sign::gen_keypair();
    Self {
      keypair: (
        pk.as_ref().try_into().unwrap(),
        sk.as_ref().try_into().unwrap()
      ),
    }
  }
}

#[async_trait::async_trait]
impl Store for MemoryStore {
  async fn get_keypair(&mut self) -> Result<Option<Keypair>,Error> {
    Ok(Some(self.keypair.clone()))
  }
  async fn set_keypair(&mut self, keypair: Keypair) -> Result<(),Error> {
    self.keypair = keypair;
    Ok(())
  }
  async fn get_latest(&mut self, channel: &[u8]) -> Result<[u8;32],Error> {
    Ok([0;32])
  }
}
