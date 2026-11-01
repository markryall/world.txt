mod client;
mod protocol;
mod server;

use clap::{Parser, Subcommand};

/// world.txt — a cozy text world. No battles, just vibes.
#[derive(Parser)]
#[command(name = "world.txt", version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the world.
    Serve {
        #[arg(long, default_value = "0.0.0.0:4040")]
        addr: String,
        #[arg(long, default_value = "world.db")]
        db: String,
        /// Seed for a brand-new world's landscape.
        #[arg(long, default_value_t = 7)]
        seed: u64,
        /// Ask everyone for this password before they can join.
        #[arg(long, env = "WORLD_PASSWORD")]
        password: Option<String>,
    },
    /// Wander in.
    Play {
        /// Who you are.
        name: Option<String>,
        /// ws://host:port, or wss://... through a tunnel.
        #[arg(long, env = "WORLD_SERVER", default_value = "ws://127.0.0.1:4040")]
        server: String,
        /// The world's password, if it has one.
        #[arg(long, env = "WORLD_PASSWORD")]
        password: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::Serve { addr, db, seed, password } => server::run(&addr, &db, seed, password).await,
        Cmd::Play { name, server, password } => client::run(name, &server, password).await,
    }
}
