mod client;
mod protocol;
mod server;

use clap::{Parser, Subcommand};

/// world.txt — a cozy text world. No battles, just vibes.
#[derive(Parser)]
#[command(version, about)]
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
    },
    /// Wander in.
    Play {
        /// Who you are.
        name: Option<String>,
        #[arg(long, default_value = "ws://127.0.0.1:4040")]
        server: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::Serve { addr, db, seed } => server::run(&addr, &db, seed).await,
        Cmd::Play { name, server } => client::run(name, &server).await,
    }
}
