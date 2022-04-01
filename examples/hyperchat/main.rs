use async_std::{io, prelude::*, task};
use cable::{Cable, ChannelOptions, MemoryStore};
use hyperswarm::{hash_topic, BootstrapNode, Config, Hyperswarm, TopicConfig};

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

const BOOTSTRAP_ADDR: &str = "127.0.0.1:6666";

fn main() -> Result<(), Error> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "debug,hyperswarm-dht=trace,hyperswarm=trace");
    }
    env_logger::init();
    let (args, _argv) = argmap::parse(std::env::args());

    task::block_on(async move {
        match args.get(1).map(|x| x.as_str()) {
            Some("join") => {
                let topic = args.get(2).expect("topic is required");
                run_chat(topic.to_owned()).await.unwrap();
            }
            Some("bootstrap") => {
                run_bootstrap().await.unwrap();
            }
            _ => {
                eprintln!("Command is either `join` or `bootstrap`");
            }
        }
    });
    Ok(())
}
async fn run_bootstrap() -> Result<(), Error> {
    let node = BootstrapNode::with_addr(BOOTSTRAP_ADDR)?;
    let (addr, task) = node.run().await?;
    eprintln!("Running bootstrap node on {:?}", addr);
    task.await?;
    Ok(())
}
async fn run_chat(topic: String) -> Result<(), Error> {
    let store = MemoryStore::default();
    let cable = Cable::new(store);
    {
        let opts = ChannelOptions {
            channel: "default".as_bytes().to_vec(),
            time_start: 0,
            //time_end: now(),
            time_end: 0,
            limit: 20,
        };
        let mut cable = cable.clone();
        task::spawn(async move {
            let mut msg_stream = cable.open_channel(&opts).await.unwrap();
            while let Some(msg) = msg_stream.next().await {
                println!["msg={:?}", msg];
            }
        });
    }
    {
        let mut cable = cable.clone();
        task::spawn(async move {
            let stdin = io::stdin();
            let mut line = String::new();
            loop {
                stdin.read_line(&mut line).await.unwrap();
                if line.is_empty() {
                    break;
                }
                let channel = "default".as_bytes();
                let text = line.trim_end().as_bytes();
                cable.post_text(channel, &text).await.unwrap();
                line.clear();
            }
        });
    }

    let topic = hash_topic(b"example:hyperchat", topic.as_bytes());
    let config = Config::default().set_bootstrap_nodes(&[BOOTSTRAP_ADDR]);
    let mut swarm = Hyperswarm::bind(config).await?;
    swarm.configure(topic, TopicConfig::both());
    while let Some(stream) = swarm.next().await.transpose()? {
        cable.listen(stream).await.unwrap();
    }

    Ok(())
}
