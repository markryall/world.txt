mod agent;
mod bridge;
mod client;
mod identity;
mod protocol;
mod server;
mod service;

use clap::{Parser, Subcommand};

/// world.txt — a cozy text world. No battles, just bonhomie.
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
    /// Recent magic links: waiting, used or expired.
    Invites {
        #[arg(long, default_value = "world.db")]
        db: String,
    },
    /// Cancel someone's unused magic links (one sent to the wrong person).
    Revoke {
        /// Their nickname or username.
        name: String,
        #[arg(long, default_value = "world.db")]
        db: String,
    },
    /// Give the world a proper name (shown to visitors and other worlds).
    Rename {
        /// e.g. "Willow Bank"
        name: String,
        #[arg(long, default_value = "world.db")]
        db: String,
    },
    /// Write past weeks' discoveries into the Island Journal's archive.
    BackfillJournal {
        #[arg(long, default_value = "world.db")]
        db: String,
    },
    /// Make someone an island keeper (run on the server's machine).
    Promote {
        /// Their nickname or username.
        name: String,
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
    /// Keep the world (and its tunnel) running in the background, from login (macOS).
    Service {
        #[command(subcommand)]
        action: ServiceCmd,
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

#[derive(Subcommand)]
enum ServiceCmd {
    /// Start the world now and at every login, restarting it if it falls over.
    Install {
        #[arg(long, default_value = "world.db")]
        db: String,
        /// The world's public address (e.g. https://world.example.com).
        #[arg(long, env = "WORLD_URL")]
        url: Option<String>,
        #[arg(long, env = "WORLD_BACKUP_DIR")]
        backup_dir: Option<std::path::PathBuf>,
        /// Also run this named Cloudflare tunnel (e.g. world).
        #[arg(long)]
        tunnel: Option<String>,
        /// Which world this is, to run several side by side (e.g. garden).
        #[arg(long)]
        name: Option<String>,
        /// The port this world listens on (each world needs its own).
        #[arg(long, default_value_t = 4040)]
        port: u16,
    },
    /// Stop a world and take it out of login.
    Uninstall {
        /// Which world (leave out for the unnamed one).
        #[arg(long, conflicts_with = "all")]
        name: Option<String>,
        /// Every installed world.
        #[arg(long)]
        all: bool,
    },
    /// Are they running?
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::Serve { addr, db, seed, keep_backups, backup_dir, url } => {
            server::run(&addr, &db, seed, keep_backups, backup_dir, url).await
        }
        Cmd::Backup { db, keep_backups, backup_dir } => {
            let made = server::backup::backup(&db, backup_dir.as_deref(), keep_backups, true)?;
            if let Some(p) = made {
                println!("💾 backed up the world to {}", p.display());
            }
            Ok(())
        }
        Cmd::Play { server, invite } => client::run(&server, invite).await,
        Cmd::Agent { name, persona, server, daily_usd, state_dir, dry_run, invite } => {
            let persona = persona.unwrap_or_else(|| format!("personas/{}.md", name.to_lowercase()).into());
            agent::run(agent::Options { name, persona, server, daily_usd, state_dir, dry_run, invite }).await
        }
        Cmd::Bridge { name, server, dir, invite } => bridge::run(name, &server, dir, invite).await,
        Cmd::Invite { nickname, keeper, url, db } => server::invite(&db, &nickname, keeper, url.as_deref()),
        Cmd::Link { name, url, db } => server::link(&db, &name, url.as_deref()),
        Cmd::Invites { db } => server::invites(&db),
        Cmd::Revoke { name, db } => server::revoke(&db, &name),
        Cmd::Rename { name, db } => server::rename(&db, &name),
        Cmd::Promote { name, db } => server::promote(&db, &name),
        Cmd::BackfillJournal { db } => server::backfill_journal(&db),
        Cmd::Service { action } => match action {
            ServiceCmd::Install { db, url, backup_dir, tunnel, name, port } => {
                service::install(service::Install { db, url, backup_dir, tunnel, name, port })
            }
            ServiceCmd::Uninstall { name, all } => service::uninstall(name, all),
            ServiceCmd::Status => service::status(),
        },
        Cmd::Send { name, command, dir } => bridge::send(&name, &dir, &command).await,
    }
}
