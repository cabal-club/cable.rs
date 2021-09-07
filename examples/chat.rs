use async_std::{prelude::*,task,net::TcpListener};
use cable::{Cable,MemoryStore};

type Error = Box<dyn std::error::Error+Send+Sync>;

fn main() -> Result<(),Error> {
  task::block_on(async move {
    //let (args,argv) = argmap::parse(std::env::args());
    let store = MemoryStore::default();
    let cable = Cable::new(Box::new(store));

    //let stream = TcpStream::connect("127.0.0.1:5000").await?;
    //cable.connect(Box::new(stream));

    let listener = TcpListener::bind("0.0.0.0:5000").await?;
    let mut incoming = listener.incoming();
    while let Some(rstream) = incoming.next().await {
      let stream = Box::new(rstream?);
      let client = cable.client();
      task::spawn_local(async move {
        client.listen(stream).await.unwrap();
      });
    }
    Ok(())
  })
}
