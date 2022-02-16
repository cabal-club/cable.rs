use async_std::{prelude::*,io,task,net};
use cable::{Cable,MemoryStore,ChannelOptions};

type Error = Box<dyn std::error::Error+Send+Sync+'static>;

fn main() -> Result<(),Error> {
  let (args,argv) = argmap::parse(std::env::args());

  task::block_on(async move {
    let store = MemoryStore::default();
    let cable = Cable::new(Box::new(store));
    {
      let client = cable.clone();
      let options = ChannelOptions {
        channel: "default".as_bytes().to_vec(),
        time_start: 0,
        //time_end: now(),
        time_end: 0,
        limit: 20,
      };
      client.open_channel(&options).await?;
      task::spawn(async move {
        let stdin = io::stdin();
        let mut line = String::new();
        loop {
          stdin.read_line(&mut line).await.unwrap();
          if line.is_empty() { break }
          let channel = "default".as_bytes();
          let text = line.trim_end().as_bytes();
          client.post_text(channel, &text).await.unwrap();
        }
      });
    }

    if let Some(port) = argv.get("l").and_then(|x| x.first()) {
      let listener = net::TcpListener::bind(format!["0.0.0.0:{}",port]).await?;
      let mut incoming = listener.incoming();
      while let Some(rstream) = incoming.next().await {
        let stream = rstream.unwrap();
        let client = cable.clone();
        task::spawn(async move {
          client.listen(stream).await.unwrap();
        });
      }
    } else if let Some(addr) = args.get(1) {
      let stream = net::TcpStream::connect(addr).await?;
      cable.listen(stream).await?;
    }
    Ok(())
  })
}

fn now() -> u64 {
  std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}
