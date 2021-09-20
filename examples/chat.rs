use async_std::{prelude::*,io,task,net::TcpListener};
use cable::{Cable,MemoryStore};

type Error = Box<dyn std::error::Error+Send+Sync+'static>;

fn main() -> Result<(),Error> {
  task::block_on(async move {
    let store = MemoryStore::default();
    let cable = Cable::new(Box::new(store));
    {
      let client = cable.client();
      task::spawn(async move {
        let stdin = io::stdin();
        let mut line = String::new();
        loop {
          println!["read_line"];
          stdin.read_line(&mut line);
          if line.is_empty() { break }
          println!["line={}", &line];
          let channel = "default".as_bytes();
          let text = line.as_bytes();
          client.post_text(channel, &text).await.unwrap();
        }
      });
    }

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
