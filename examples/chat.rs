use async_std::{prelude::*,task,net::TcpListener,stream::Stream};
use cable::{Cable,MemoryStore};

type Error = Box<dyn std::error::Error+Send+Sync+'static>;

fn main() -> Result<(),Error> {
  task::block_on(async move {
    let store = MemoryStore::default();
    let cable = Cable::new(Box::new(store));

    let listener = TcpListener::bind("0.0.0.0:5000").await?;
    let mut incoming = listener.incoming();
    while let Some(rstream) = incoming.next().await {
      let stream = rstream.unwrap();
      let client = cable.client();
      task::spawn(async move {
        client.listen(stream).await.unwrap();
      });
    }
    Ok(())
  })
}
