//! Networking. Each connection is a small task that shuttles messages
//! between its websocket and the single game task that owns the world.

pub mod accounts;
pub mod backup;
mod db;
mod game;
pub mod federation;
pub mod islands;
pub mod names;
mod nature;
mod rng;
mod terrain;
mod tutorial;
mod weather;
mod world;

use crate::protocol::{ToClient, ToServer};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;

pub type ClientTx = UnboundedSender<ToClient>;

pub enum Event {
    Join { id: u64, bot: bool, invite: Option<String>, token: Option<String>, tx: ClientTx },
    /// Someone from another world, with a checked passport.
    Visit { id: u64, passport: federation::Passport, tx: ClientTx },
    Cmd { id: u64, text: String },
    Leave { id: u64 },
}

pub async fn run(
    addr: &str,
    db_path: &str,
    seed: u64,
    keep_backups: usize,
    backup_dir: Option<std::path::PathBuf>,
    url: Option<String>,
) -> anyhow::Result<()> {
    let db = db::Db::open(db_path)?;
    let world = world::World::load_or_create(db, seed)?;
    // This world's public face, for other worlds checking its passports.
    let info: Arc<Option<federation::WorldInfo>> =
        Arc::new(url.as_deref().map(|u| federation::WorldInfo::new(u, &federation::world_key(&world.db).expect("a signing key"), world.db.meta(federation::NAME_KEY))));
    let (events_tx, events_rx) = mpsc::unbounded_channel();
    if world.db.keeper_count() == 0 {
        println!("🗝  No island keepers yet. Make yourself one: world.txt invite <YourName> --keeper --db {db_path}");
    }
    if let Some(i) = info.as_ref() {
        println!("🌍 this world is {} (other worlds can check its passports at {}{})", i.name, i.url, federation::WELL_KNOWN);
    }
    tokio::spawn(game::run(world, events_rx, url));
    if keep_backups > 0 {
        tokio::spawn(backup::run_daily(db_path.to_string(), backup_dir, keep_backups));
    }

    let listener = TcpListener::bind(addr).await?;
    println!("🌍 world.txt is open: browser http://{addr}  ·  terminal ws://{addr}  (db: {db_path})");
    println!("💌 invite-only: world.txt invite <Name> makes an account and its magic link");
    let mut next_id = 1u64;
    loop {
        let (stream, peer) = listener.accept().await?;
        let id = next_id;
        next_id += 1;
        let events = events_tx.clone();
        let info = info.clone();
        tokio::spawn(async move {
            if let Err(e) = connection(id, stream, events, info).await {
                eprintln!("connection {peer}: {e}");
            }
        });
    }
}

async fn connection(
    id: u64,
    stream: TcpStream,
    events: UnboundedSender<Event>,
    info: Arc<Option<federation::WorldInfo>>,
) -> anyhow::Result<()> {
    if !is_websocket(&stream).await? {
        return serve_page(stream, &info).await;
    }
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut sink, mut source) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ToClient>();

    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let bye = matches!(msg, ToClient::Bye { .. });
            let json = serde_json::to_string(&msg).expect("serialise");
            if sink.send(Message::Text(json.into())).await.is_err() || bye {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let mut joined = false;
    while let Some(msg) = source.next().await {
        let text = match msg? {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };
        let Ok(parsed) = serde_json::from_str::<ToServer>(text.as_str()) else {
            continue;
        };
        match parsed {
            ToServer::Hello { passport: Some(p), .. } if !joined => {
                // Check it here, so the game never waits on another world.
                let checked = match info.as_ref() {
                    Some(me) => federation::verify(&p, &me.url, accounts::now()).await,
                    None => Err(anyhow::anyhow!("this world doesn't take visitors")),
                };
                match checked {
                    Ok(passport) => {
                        joined = true;
                        events.send(Event::Visit { id, passport, tx: tx.clone() })?;
                    }
                    Err(e) => {
                        let _ = tx.send(ToClient::Bye { reason: format!("🛂 Sorry: {e}.") });
                        break;
                    }
                }
            }
            ToServer::Hello { bot, invite, token, .. } if !joined => {
                joined = true;
                events.send(Event::Join { id, bot, invite, token, tx: tx.clone() })?;
            }
            ToServer::Cmd { text } if joined => {
                events.send(Event::Cmd { id, text })?;
            }
            _ => {}
        }
    }
    if joined {
        let _ = events.send(Event::Leave { id });
    }
    drop(tx);
    let _ = writer.await;
    Ok(())
}

/// One port, two doors: websocket upgrades go to the game, anything else
/// gets the browser client.
async fn is_websocket(stream: &TcpStream) -> anyhow::Result<bool> {
    let mut buf = [0u8; 4096];
    for _ in 0..50 {
        let n = stream.peek(&mut buf).await?;
        let head = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
        if head.contains("upgrade: websocket") {
            return Ok(true);
        }
        if head.contains("\r\n\r\n") || n == buf.len() {
            return Ok(false);
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    Ok(false)
}

const PAGE: &str = include_str!("../../web/index.html");

/// The page, wearing the world's name if it has one.
fn page(info: &Option<federation::WorldInfo>) -> String {
    match info {
        Some(i) if i.name != federation::host(&i.url) => {
            let name = i.name.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            PAGE.replacen("<title>world.txt</title>", &format!("<title>{name} · world.txt</title>"), 1)
                .replacen("<h1>✨ world.txt ✨</h1>", &format!("<h1>✨ {name} ✨</h1>"), 1)
        }
        _ => PAGE.to_string(),
    }
}

async fn serve_page(mut stream: TcpStream, info: &Option<federation::WorldInfo>) -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req.split_whitespace().nth(1).unwrap_or("/");
    let (status, ctype, body) = match (path, info) {
        ("/" | "/index.html", _) => ("200 OK", "text/html; charset=utf-8", page(info)),
        (federation::WELL_KNOWN, Some(info)) => ("200 OK", "application/json", serde_json::to_string(info)?),
        _ => ("404 Not Found", "text/plain; charset=utf-8", "nothing here, babe\n".to_string()),
    };
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

/// `world.txt rename <name>`: give the world a proper name.
pub fn rename(db_path: &str, name: &str) -> anyhow::Result<()> {
    let db = db::Db::open(db_path)?;
    let name = federation::clean_world_name(name);
    if name.is_empty() {
        anyhow::bail!("a world name needs a letter or two, e.g. world.txt rename \"Willow Bank\"");
    }
    db.set_meta(federation::NAME_KEY, &name)?;
    println!("🌍 this world is called {name} now. Restart it (or run service install again) for it to show.");
    Ok(())
}

/// `world.txt invite <nickname> [--keeper]`: a new account and its magic link.
pub fn invite(db_path: &str, nickname: &str, keeper: bool, url: Option<&str>) -> anyhow::Result<()> {
    let db = db::Db::open(db_path)?;
    let nick = names_clean(nickname);
    if nick.is_empty() {
        anyhow::bail!("nicknames need a letter or two");
    }
    if !db.matching_users(&nick)?.is_empty() {
        anyhow::bail!("someone's already called {nick}. For their new device: world.txt link {nick}");
    }
    let user = accounts::create_account(&db, &nick, keeper)?;
    let token = accounts::issue_invite(&db, &user)?;
    let role = if keeper { " (island keeper)" } else { "" };
    println!("💌 {nick} is {user}{role}. Their magic link (one use, within {} days):", accounts::INVITE_DAYS);
    println!("   {}", accounts::link(url, &token));
    if url.is_none() {
        println!("   (replace <world address> with how they reach the world, or pass --url / WORLD_URL)");
    }
    println!("   In a terminal: world.txt play --invite {token}");
    Ok(())
}

/// `world.txt link <name>`: a new magic link for someone's next device.
pub fn link(db_path: &str, name: &str, url: Option<&str>) -> anyhow::Result<()> {
    let db = db::Db::open(db_path)?;
    let user = one_user(&db, name)?;
    let token = accounts::issue_invite(&db, &user)?;
    println!("💌 A new link for {} ({user}), one use, within {} days:", db.nickname(&user).unwrap_or_default(), accounts::INVITE_DAYS);
    println!("   {}", accounts::link(url, &token));
    Ok(())
}

/// The one account a name means, or why not.
fn one_user(db: &db::Db, name: &str) -> anyhow::Result<String> {
    let found = db.matching_users(name)?;
    match found.as_slice() {
        [] => anyhow::bail!("there's nobody called {name}"),
        [one] => Ok(one.clone()),
        many => anyhow::bail!("'{name}' could be any of: {} (use their username)", many.join(", ")),
    }
}

fn names_clean(raw: &str) -> String {
    let n: String = raw.trim().chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_').take(16).collect();
    let mut c = n.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// `world.txt promote <name>`: the server's machine can always make a keeper.
pub fn promote(db_path: &str, name: &str) -> anyhow::Result<()> {
    let db = db::Db::open(db_path)?;
    let user = one_user(&db, name)?;
    db.set_keeper(&user, true)?;
    println!("🗝  {} ({user}) is an island keeper now.", db.nickname(&user).unwrap_or_default());
    Ok(())
}

/// `world.txt backfill-journal`: write past weeks' discoveries into an
/// existing Island Journal (safe to run again).
pub fn backfill_journal(db_path: &str) -> anyhow::Result<()> {
    let db = db::Db::open(db_path)?;
    let Some(board) = db.things()?.into_iter().find(|t| t.kind.as_deref() == Some("journal")).map(|t| t.id) else {
        println!("There's no Island Journal yet: it's put up (and filled in) the first time the world starts.");
        return Ok(());
    };
    let added = game::backfill_journal(&db, board)?;
    println!("📖 wrote {added} past discoveries into the Island Journal (earlier weeks are in its archive).");
    Ok(())
}
