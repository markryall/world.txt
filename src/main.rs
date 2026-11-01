mod agent;
mod bridge;
mod client;
mod identity;
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
        /// Island keepers (comma-separated names) can tidy up anyone's things.
        #[arg(long, env = "WORLD_KEEPERS", value_delimiter = ',')]
        keeper: Vec<String>,
        /// Daily copies to keep (0 turns backups off).
        #[arg(long, default_value_t = 14)]
        keep_backups: usize,
        /// Where daily copies go (default: backups/ next to the db). Somewhere
        /// off this disk is best, e.g. an iCloud Drive folder or an external drive.
        #[arg(long, env = "WORLD_BACKUP_DIR")]
        backup_dir: Option<std::path::PathBuf>,
        /// The world's public address, so invite links come out whole.
        #[arg(long, env = "WORLD_URL")]
        url: Option<String>,
    },
    /// Copy the world to backups/ right now.
    Backup {
        #[arg(long, default_value = "world.db")]
        db: String,
        #[arg(long, default_value_t = 14)]
        keep_backups: usize,
        #[arg(long, env = "WORLD_BACKUP_DIR")]
        backup_dir: Option<std::path::PathBuf>,
    },
    /// Wander in.
    Play {
        /// ws://host:port, or wss://... through a tunnel.
        #[arg(long, env = "WORLD_SERVER", default_value = "ws://127.0.0.1:4040")]
        server: String,
        /// The world's password, if it has one.
        #[arg(long, env = "WORLD_PASSWORD")]
        password: Option<String>,
        /// An invite (the part after #invite= in a magic link), the first time.
        #[arg(long)]
        invite: Option<String>,
    },
    /// Bring an AI island resident to life (uses Claude Haiku 5.5).
    Agent {
        /// The resident's name, e.g. Moss.
        name: String,
        /// Character sheet. Defaults to personas/<name>.md.
        #[arg(long)]
        persona: Option<std::path::PathBuf>,
        #[arg(long, env = "WORLD_SERVER", default_value = "ws://127.0.0.1:4040")]
        server: String,
        #[arg(long, env = "WORLD_PASSWORD")]
        password: Option<String>,
        /// Most it may spend in a day, in US dollars (estimated from API usage).
        #[arg(long, env = "WORLD_AGENT_DAILY_USD", default_value_t = 0.50)]
        daily_usd: f64,
        /// Where spend and memories are kept.
        #[arg(long, default_value = "agent-state")]
        state_dir: std::path::PathBuf,
        /// Show what it would send to Claude, without calling the API.
        #[arg(long)]
        dry_run: bool,
        /// An invite (the part after #invite= in a magic link), the first time.
        #[arg(long)]
        invite: Option<String>,
    },
    /// Keep a resident connected for something else to drive (e.g. a Claude Code session).
    Bridge {
        name: String,
        #[arg(long, env = "WORLD_SERVER", default_value = "ws://127.0.0.1:4040")]
        server: String,
        #[arg(long, env = "WORLD_PASSWORD")]
        password: Option<String>,
        /// Where the log and socket live.
        #[arg(long, default_value = "tmp/bridge")]
        dir: std::path::PathBuf,
        /// An invite (the part after #invite= in a magic link), the first time.
        #[arg(long)]
        invite: Option<String>,
    },
    /// Make an account and print its magic link (run on the server's machine).
    Invite {
        /// Their nickname, e.g. Poppy.
        nickname: String,
        /// Make them an island keeper (do this for yourself first).
        #[arg(long)]
        keeper: bool,
        /// The world's public address, for the link (e.g. https://world.example.com).
        #[arg(long, env = "WORLD_URL")]
        url: Option<String>,
        #[arg(long, default_value = "world.db")]
        db: String,
    },
    /// Print a new magic link for someone's next device.
    Link {
        /// Their nickname or username.
        name: String,
        #[arg(long, env = "WORLD_URL")]
        url: Option<String>,
        #[arg(long, default_value = "world.db")]
        db: String,
    },
    /// Free a name so a new device can claim it (e.g. a kid on a new iPad).
    Release {
        name: String,
        #[arg(long, default_value = "world.db")]
        db: String,
    },
    /// Let a kid's new device into their name, using the code it was shown.
    Welcome {
        name: String,
        /// The code on their screen, e.g. FROG-42.
        code: String,
        #[arg(long, default_value = "world.db")]
        db: String,
    },
    /// Send one command through a running bridge and print what comes back.
    Send {
        name: String,
        /// The command, e.g. say hello there
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
        #[arg(long, default_value = "tmp/bridge")]
        dir: std::path::PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::Serve { addr, db, seed, password, keeper, keep_backups, backup_dir, url } => {
            server::run(&addr, &db, seed, password, keeper, keep_backups, backup_dir, url).await
        }
        Cmd::Backup { db, keep_backups, backup_dir } => {
            let made = server::backup::backup(&db, backup_dir.as_deref(), keep_backups, true)?;
            if let Some(p) = made {
                println!("💾 backed up the world to {}", p.display());
            }
            Ok(())
        }
        Cmd::Play { server, password, invite } => client::run(&server, password, invite).await,
        Cmd::Agent { name, persona, server, password, daily_usd, state_dir, dry_run, invite } => {
            let persona = persona.unwrap_or_else(|| format!("personas/{}.md", name.to_lowercase()).into());
            agent::run(agent::Options { name, persona, server, password, daily_usd, state_dir, dry_run, invite }).await
        }
        Cmd::Bridge { name, server, password, dir, invite } => bridge::run(name, &server, password, dir, invite).await,
        Cmd::Invite { nickname, keeper, url, db } => server::invite(&db, &nickname, keeper, url.as_deref()),
        Cmd::Link { name, url, db } => server::link(&db, &name, url.as_deref()),
        Cmd::Send { name, command, dir } => bridge::send(&name, &dir, &command).await,
        Cmd::Release { name, db } => server::release(&db, &name),
        Cmd::Welcome { name, code, db } => server::welcome(&db, &name, &code),
    }
}
