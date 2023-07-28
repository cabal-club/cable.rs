//! An example chat application over a TCP stream.
//!
//! Run the example as a TCP server in one terminal:
//!
//! `cargo run --example chat -- -l 8008`
//!
//! And then connect to the server in a second terminal:
//!
//! `cargo run --example chat -- 0.0.0.0:8008`
//!
//! Write text to either terminal and press <Enter> to post.

use async_std::{io, net, prelude::*, task};

use cable::ChannelOptions;
use cable_core::{CableManager, MemoryStore};

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn main() -> Result<(), Error> {
    let (args, argv) = argmap::parse(std::env::args());

    task::block_on(async move {
        let store = MemoryStore::default();
        let cable = CableManager::new(store);

        let opts = ChannelOptions {
            channel: "default".to_string(),
            time_start: now(),
            time_end: 0,
            limit: 20,
        };

        // Open a channel and print received messages.
        let mut client = cable.clone();
        task::spawn(async move {
            let mut msg_stream = client.open_channel(&opts).await.unwrap();
            while let Some(msg) = msg_stream.next().await {
                if let Ok(m) = msg {
                    println!["{}", m];
                }
            }
        });

        // Read text from `stdin` and post it to the default channel.
        let mut client = cable.clone();
        task::spawn(async move {
            let stdin = io::stdin();
            let mut line = String::new();
            loop {
                stdin.read_line(&mut line).await.unwrap();
                if line.is_empty() {
                    break;
                }
                let channel = "default".to_string();
                let text = line.trim_end();
                client.post_text(channel, text).await.unwrap();
                line.clear();
            }
        });

        // Deploy a TCP listener and pass the stream to the cable manager.
        if let Some(port) = argv.get("l").and_then(|x| x.first()) {
            println!("Deploying TCP server on 0.0.0.0:{}", port);
            let listener = net::TcpListener::bind(format!["0.0.0.0:{}", port]).await?;
            let mut incoming = listener.incoming();
            while let Some(stream) = incoming.next().await {
                let stream = stream.unwrap();
                let client = cable.clone();
                task::spawn(async move {
                    client.listen(stream).await.unwrap();
                });
            }
        // Connect to a TCP server and pass the stream to the cable manager.
        } else if let Some(addr) = args.get(1) {
            println!("Connecting to TCP server on {}", addr);
            let stream = net::TcpStream::connect(addr).await?;
            cable.listen(stream).await?;
        }

        Ok(())
    })
}
