//! Networking. Each connection is a small task that shuttles messages
//! between its websocket and the single game task that owns the world.

pub mod accounts;
pub mod backup;
mod db;
mod game;
pub mod federation;
mod gardens;
pub mod islands;
pub mod names;
mod nature;
pub mod paint;
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

/// Music waiting to go out to one listener before more is dropped: about
/// five seconds' worth (a batch is 200ms).
pub const MAX_AUDIO_QUEUED: usize = 25;

/// A player's outgoing messages, with limits so a connection that's fallen
/// behind (a phone gone to sleep, a slow network, a rush of arrivals) can't pile
/// messages up in memory until the whole world runs out:
/// - music past a few seconds' worth is dropped;
/// - side panels are skipped once a few dozen messages are waiting (a fresh
///   one follows within a tick anyway);
/// - past [`MAX_QUEUED`], the connection is closed and its queue let go. The
///   browser reconnects on its own and catches up from the start.
#[derive(Clone)]
pub struct ClientTx {
    tx: UnboundedSender<ToClient>,
    audio_queued: Arc<std::sync::atomic::AtomicUsize>,
    queued: Arc<std::sync::atomic::AtomicUsize>,
    swamped: Arc<std::sync::atomic::AtomicBool>,
}

/// Messages waiting for one connection before side panels are skipped.
pub const STATUS_SKIP_QUEUED: usize = 50;
/// Messages waiting for one connection before it's let go.
pub const MAX_QUEUED: usize = 500;

impl ClientTx {
    pub fn new(tx: UnboundedSender<ToClient>) -> ClientTx {
        ClientTx { tx, audio_queued: Arc::default(), queued: Arc::default(), swamped: Arc::default() }
    }

    /// Queue a message. The error (the connection's gone) carries the message
    /// back, boxed, since some messages are big.
    pub fn send(&self, msg: ToClient) -> Result<(), Box<mpsc::error::SendError<ToClient>>> {
        use std::sync::atomic::Ordering::Relaxed;
        if self.swamped.load(Relaxed) {
            return Ok(());
        }
        let queued = self.queued.load(Relaxed);
        if queued >= MAX_QUEUED {
            self.swamped.store(true, Relaxed);
            return Ok(());
        }
        match msg {
            ToClient::Audio { .. } => {
                if self.audio_queued.load(Relaxed) >= MAX_AUDIO_QUEUED {
                    return Ok(());
                }
                self.audio_queued.fetch_add(1, Relaxed);
            }
            ToClient::Status { .. } if queued >= STATUS_SKIP_QUEUED => return Ok(()),
            _ => {}
        }
        self.queued.fetch_add(1, Relaxed);
        self.tx.send(msg).map_err(Box::new)
    }

    /// The writer took a message off the queue.
    fn taken(&self, msg: &ToClient) {
        use std::sync::atomic::Ordering::Relaxed;
        self.queued.fetch_sub(1, Relaxed);
        if matches!(msg, ToClient::Audio { .. }) {
            self.audio_queued.fetch_sub(1, Relaxed);
        }
    }

    /// Fallen too far behind: time to let this connection go.
    fn swamped(&self) -> bool {
        self.swamped.load(std::sync::atomic::Ordering::Relaxed)
    }
}

pub enum Event {
    Join { id: u64, bot: bool, invite: Option<String>, token: Option<String>, back: bool, tz: Option<String>, tx: ClientTx },
    /// Someone from another world, with a checked passport.
    Visit { id: u64, passport: federation::Passport, tz: Option<String>, tx: ClientTx },
    Cmd { id: u64, text: String },
    /// Music from a browser playing a gramophone.
    Audio { id: u64, frames: Vec<String>, title: Option<String> },
    /// Songs picked for a jukebox, and a jukebox song finishing.
    Queue { id: u64, thing: i64, songs: Vec<crate::protocol::Song> },
    TrackDone { id: u64, thing: i64, key: String },
    /// What a radio's on, looked up elsewhere: None if nothing's known.
    RadioNews { thing: i64, title: Option<String> },
    Leave { id: u64 },
    /// Something an outside tool posted to a board's hook.
    Hook { secret: String, text: String, reply: tokio::sync::oneshot::Sender<HookReply> },
    /// The server's been asked to stop (a redeploy): say so, then go.
    Shutdown,
}

/// How a hook's post went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookReply {
    Posted,
    /// No such hook (or it's been stopped, or its board is gone).
    Unknown,
    /// The same hook posted again too soon.
    TooFast,
    /// Moved a progress bar along, or set a status light (or cleared one).
    Progressed,
    /// A progress hook got something that isn't progress.
    NotProgress,
    /// A status hook got something that isn't up or down.
    NotStatus,
}

pub async fn run(
    addr: &str,
    db_path: &str,
    seed: Option<u64>,
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
    tokio::spawn(game::run(world, events_rx, events_tx.clone(), url));
    if keep_backups > 0 {
        tokio::spawn(backup::run_daily(db_path.to_string(), backup_dir, keep_backups));
    }

    // Stopping (Ctrl-C, or a host like Fly.io redeploying): tell everyone it's
    // only for a moment, so their browsers wait and reconnect.
    {
        let events = events_tx.clone();
        tokio::spawn(async move {
            stopping().await;
            let _ = events.send(Event::Shutdown);
        });
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

/// Waits for Ctrl-C (SIGINT) or SIGTERM.
async fn stopping() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

async fn connection(
    id: u64,
    stream: TcpStream,
    events: UnboundedSender<Event>,
    info: Arc<Option<federation::WorldInfo>>,
) -> anyhow::Result<()> {
    if !is_websocket(&stream).await? {
        return serve_page(stream, &info, &events).await;
    }
    let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default().max_message_size(Some(MAX_MESSAGE)).max_frame_size(Some(MAX_MESSAGE));
    let ws = tokio_tungstenite::accept_async_with_config(stream, Some(config)).await?;
    let (mut sink, mut source) = ws.split();
    let (raw_tx, mut rx) = mpsc::unbounded_channel::<ToClient>();
    let tx = ClientTx::new(raw_tx);
    let counter = tx.clone();

    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            counter.taken(&msg);
            if counter.swamped() {
                eprintln!("connection {id}: fell too far behind, letting it go");
                break;
            }
            let bye = matches!(msg, ToClient::Bye { .. });
            let json = serde_json::to_string(&msg).expect("serialise");
            let sent = sink.send(Message::Text(json.into())).await;
            if sent.is_err() || bye {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let mut joined = false;
    let mut pace = Pace::new(std::time::Instant::now());
    while let Some(msg) = source.next().await {
        // A connection that drops without a goodbye (a phone losing signal)
        // is an error here: it still has to leave, not linger as a ghost.
        let text = match msg {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => continue,
        };
        // Saying hello takes a few hundred bytes: anything much bigger from
        // someone who hasn't yet is up to no good.
        if !joined && text.len() > MAX_HELLO {
            break;
        }
        let Ok(parsed) = serde_json::from_str::<ToServer>(text.as_str()) else {
            continue;
        };
        match parsed {
            ToServer::Hello { passport: Some(p), tz, .. } if !joined => {
                // Check it here, so the game never waits on another world.
                let checked = match info.as_ref() {
                    Some(me) => federation::verify(&p, &me.url, accounts::now()).await,
                    None => Err(anyhow::anyhow!("this world doesn't take visitors")),
                };
                match checked {
                    Ok(passport) => {
                        joined = true;
                        events.send(Event::Visit { id, passport, tz, tx: tx.clone() })?;
                    }
                    Err(e) => {
                        let _ = tx.send(ToClient::Bye { reason: format!("🛂 Sorry: {e}.") });
                        break;
                    }
                }
            }
            ToServer::Hello { bot, invite, token, back, tz, .. } if !joined => {
                joined = true;
                events.send(Event::Join { id, bot, invite, token, back, tz, tx: tx.clone() })?;
            }
            ToServer::Cmd { text } if joined => {
                if pace.take(std::time::Instant::now()) {
                    events.send(Event::Cmd { id, text })?;
                } else if pace.warn() {
                    let line = crate::protocol::Line::col(crate::protocol::Col::Dim, "🐢 Slow down a little: the island can't keep up. Try again in a moment.");
                    let _ = tx.send(ToClient::Lines { lines: vec![line] });
                }
            }
            ToServer::Audio { frames, title } if joined => {
                events.send(Event::Audio { id, frames, title })?;
            }
            ToServer::Queue { thing, songs } if joined => {
                events.send(Event::Queue { id, thing, songs })?;
            }
            ToServer::TrackDone { thing, key } if joined => {
                events.send(Event::TrackDone { id, thing, key })?;
            }
            _ => {}
        }
    }
    if joined {
        let _ = events.send(Event::Leave { id });
    }
    drop(tx);
    // The writer holds a sender of its own (to count what's queued), so its
    // queue never runs dry by itself: give it a moment to send any last
    // goodbye, then let it go, and the connection with it.
    let mut writer = writer;
    if tokio::time::timeout(std::time::Duration::from_secs(2), &mut writer).await.is_err() {
        writer.abort();
    }
    Ok(())
}

/// Commands someone can send in a rush before they're asked to slow down.
const PACE_BURST: f64 = 20.0;
/// How many a second they can keep up after that.
const PACE_PER_SEC: f64 = 4.0;

/// A gentle speed limit on one connection's commands (a token bucket):
/// plenty for anyone typing, or a script, but not a flood.
struct Pace {
    tokens: f64,
    at: std::time::Instant,
    warned: bool,
}

impl Pace {
    fn new(now: std::time::Instant) -> Pace {
        Pace { tokens: PACE_BURST, at: now, warned: false }
    }

    /// Whether a command may go through now.
    fn take(&mut self, now: std::time::Instant) -> bool {
        self.tokens = (self.tokens + now.duration_since(self.at).as_secs_f64() * PACE_PER_SEC).min(PACE_BURST);
        self.at = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            self.warned = false;
            true
        } else {
            false
        }
    }

    /// Whether to say so: once each time they run out, not for every command dropped.
    fn warn(&mut self) -> bool {
        !std::mem::replace(&mut self.warned, true)
    }
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

/// The most a request may send: its headers, and a hook's post.
/// The biggest message a browser sends: half a second or so of a
/// gramophone's music (at most 60 frames of 4KB), or a jukebox's song list.
const MAX_MESSAGE: usize = 512 * 1024;
/// The biggest message before saying hello (a passport is the longest).
const MAX_HELLO: usize = 8 * 1024;
const MAX_HEAD: usize = 8 * 1024;
const MAX_BODY: usize = 16 * 1024;

/// A plain HTTP request, as much of it as the world cares about.
#[derive(Debug, PartialEq)]
struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

enum ReadError {
    TooBig,
    Bad,
}

/// Read one request: headers, then a body of up to [`MAX_BODY`] bytes.
async fn read_request(stream: &mut TcpStream) -> Result<Request, ReadError> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > MAX_HEAD {
            return Err(ReadError::TooBig);
        }
        let n = stream.read(&mut chunk).await.map_err(|_| ReadError::Bad)?;
        if n == 0 {
            if buf.is_empty() {
                return Err(ReadError::Bad);
            }
            break buf.len(); // no blank line: take what came
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut first = head.lines().next().unwrap_or("").split_whitespace();
    let method = first.next().unwrap_or("GET").to_ascii_uppercase();
    let path = first.next().unwrap_or("/").to_string();
    let length = head
        .lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if length > MAX_BODY {
        return Err(ReadError::TooBig);
    }
    let mut body = buf[head_end..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut chunk).await.map_err(|_| ReadError::Bad)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    Ok(Request { method, path, body })
}

/// What a hook's post says: JSON with "text" (Slack style), "content"
/// (Discord style) or "message", or else the body as plain text.
fn hook_text(body: &[u8]) -> Option<String> {
    let raw = String::from_utf8_lossy(body);
    let raw = raw.trim();
    let text = if raw.starts_with('{') {
        let v: serde_json::Value = serde_json::from_str(raw).ok()?;
        let s = |k: &str| v.get(k).and_then(|t| t.as_str()).map(str::to_string);
        // Pingdom-style alerts: {"current_state": "DOWN", "description": "…"}.
        let state = s("current_state").map(|st| format!("{st} {}", s("description").or_else(|| s("check_name")).unwrap_or_default()));
        ["text", "content", "message"].iter().find_map(|k| s(k)).or(state).or_else(|| semaphore(&v))?
    } else {
        raw.to_string()
    };
    let text = crate::links::from_slack(&text);
    // One tidy line: no control characters, runs of space squashed.
    let text: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

/// Semaphore's notification webhook: {"pipeline": {"result": "failed"},
/// "project": {"name": …}, "revision": {"branch": {"name": …}}, …}, said as
/// "failed [web main](https://<org>.semaphoreci.com/workflows/<id>): the commit".
fn semaphore(v: &serde_json::Value) -> Option<String> {
    let s = |path: &str| v.pointer(path).and_then(|t| t.as_str()).filter(|t| !t.is_empty());
    let result = s("/pipeline/result")?;
    let what = [s("/project/name"), s("/revision/branch/name").or(s("/revision/tag/name"))].into_iter().flatten().collect::<Vec<_>>().join(" ");
    let what = match (s("/organization/name"), s("/workflow/id")) {
        (Some(org), Some(id)) if !what.is_empty() => format!("[{what}](https://{org}.semaphoreci.com/workflows/{id})"),
        _ => what,
    };
    let commit = s("/revision/commit_message").and_then(|m| m.lines().next()).map(|m| format!(": {m}")).unwrap_or_default();
    Some(format!("{result} {what}{commit}"))
}

async fn serve_page(mut stream: TcpStream, info: &Option<federation::WorldInfo>, events: &UnboundedSender<Event>) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;
    let read = tokio::time::timeout(std::time::Duration::from_secs(10), read_request(&mut stream)).await;
    let text = |status: &'static str, body: &str| (status, "text/plain; charset=utf-8", format!("{body}\n"));
    let (status, ctype, body) = match read {
        Err(_) | Ok(Err(ReadError::Bad)) => text("400 Bad Request", "that didn't make sense, babe"),
        Ok(Err(ReadError::TooBig)) => text("413 Payload Too Large", "that's a lot, babe: keep it under 16KB"),
        Ok(Ok(req)) => match (req.method.as_str(), req.path.as_str(), info) {
            ("POST", p, _) if p.starts_with("/hook/") => {
                let secret = p.trim_start_matches("/hook/").split(['?', '#']).next().unwrap_or("").to_string();
                match hook_text(&req.body) {
                    None => text("400 Bad Request", "nothing to post: send some text, or JSON with \"text\""),
                    Some(t) => {
                        let (reply, answer) = tokio::sync::oneshot::channel();
                        events.send(Event::Hook { secret, text: t, reply })?;
                        match answer.await {
                            Ok(HookReply::Posted) => text("200 OK", "posted 📌"),
                            Ok(HookReply::Progressed) => text("200 OK", "noted ✨"),
                            Ok(HookReply::NotProgress) => text("400 Bad Request", "send how far along it is: 37/100, 37%, '3 of 10 photos', or clear"),
                            Ok(HookReply::NotStatus) => text("400 Bad Request", "send up or down (or ok, failed, passed…), with any words after it, or clear"),
                            Ok(HookReply::TooFast) => text("429 Too Many Requests", "one at a time, babe: wait a couple of seconds"),
                            Ok(HookReply::Unknown) | Err(_) => text("404 Not Found", "no hook here (it may have been stopped)"),
                        }
                    }
                }
            }
            (_, "/" | "/index.html", _) => ("200 OK", "text/html; charset=utf-8", page(info)),
            (_, federation::WELL_KNOWN, Some(info)) => ("200 OK", "application/json", serde_json::to_string(info)?),
            _ => text("404 Not Found", "nothing here, babe"),
        },
    };
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

/// `world.txt invites`: recent magic links and what became of them.
pub fn invites(db_path: &str) -> anyhow::Result<()> {
    let db = db::Db::open(db_path)?;
    let now = accounts::now();
    let list = db.invites(30)?;
    if list.is_empty() {
        println!("💌 no invites yet");
    }
    for (user, nick, expires, used_at) in list {
        println!("💌 {nick} ({user}): {}", accounts::invite_state(expires, used_at, now));
    }
    Ok(())
}

/// `world.txt revoke <name>`: cancel someone's unused links.
pub fn revoke(db_path: &str, name: &str) -> anyhow::Result<()> {
    let db = db::Db::open(db_path)?;
    let user = one_user(&db, name)?;
    let n = db.revoke_invites(&user)?;
    let nick = db.nickname(&user).unwrap_or_default();
    match n {
        0 => println!("{nick} ({user}) had no unused links."),
        1 => println!("✂️  cancelled {nick}'s unused link ({user})"),
        n => println!("✂️  cancelled {nick}'s {n} unused links ({user})"),
    }
    Ok(())
}

/// The recommended radio stations that come with world.txt.
const STATIONS: &str = include_str!("../../stations.txt");

/// The recommended stations: (name, stream, guide).
pub fn recommended_stations() -> Vec<(String, String, Option<String>)> {
    STATIONS
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut parts = l.split('|').map(str::trim);
            let name = parts.next().filter(|n| !n.is_empty())?.to_string();
            let url = parts.next().filter(|u| u.starts_with("https://"))?.to_string();
            let guide = parts.next().filter(|g| g.starts_with("https://")).map(str::to_string);
            Some((name, url, guide))
        })
        .collect()
}

/// `world.txt add-stations`: put the recommended stations on the world's list
/// (keeping any others; ones with the same name are updated).
pub fn add_stations(db_path: &str) -> anyhow::Result<()> {
    let db = db::Db::open(db_path)?;
    let list = recommended_stations();
    for (name, url, guide) in &list {
        db.set_station(&game::clean_station_name(name), url, guide.as_deref(), "world.txt", accounts::now())?;
    }
    println!("📻 {} stations on the list now. In the world: stations, then tune radio <station>", db.stations()?.len());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recommended_stations_are_all_good() {
        let list = recommended_stations();
        assert!(list.len() >= 20, "only {}", list.len());
        let mut names = std::collections::HashSet::new();
        for (name, url, guide) in &list {
            assert!(names.insert(name.to_lowercase()), "{name} twice");
            assert_eq!(name, &game::clean_station_name(name), "{name} isn't a tidy name");
            assert!(url.starts_with("https://") && !url.contains(' '), "{name}: {url}");
            assert!(guide.as_deref().is_none_or(|g| g.starts_with("https://")), "{name}");
        }
    }

    #[test]
    fn music_for_a_stalled_listener_is_dropped_but_words_still_go() {
        let (raw, mut rx) = mpsc::unbounded_channel();
        let tx = ClientTx::new(raw);
        let audio = || ToClient::Audio { thing: 1, frames: vec!["AAAA".into()], gain: 1.0, title: None };
        for _ in 0..MAX_AUDIO_QUEUED + 10 {
            tx.send(audio()).unwrap();
        }
        tx.send(ToClient::Bye { reason: "still gets through".into() }).unwrap();
        let mut queued = Vec::new();
        while let Ok(m) = rx.try_recv() {
            queued.push(m);
        }
        assert_eq!(queued.iter().filter(|m| matches!(m, ToClient::Audio { .. })).count(), MAX_AUDIO_QUEUED);
        assert!(matches!(queued.last(), Some(ToClient::Bye { .. })));
        // Once the writer catches up, music flows again.
        for m in &queued {
            tx.taken(m);
        }
        tx.send(audio()).unwrap();
        assert!(matches!(rx.try_recv(), Ok(ToClient::Audio { .. })));
    }

    fn status() -> ToClient {
        ToClient::Status { place: String::new(), weather: String::new(), clock: String::new(), map: vec![], here: vec![], humans: 1, people: vec![], progress: vec![], lights: vec![] }
    }
    fn words() -> ToClient {
        ToClient::Lines { lines: vec![] }
    }

    #[test]
    fn side_panels_are_skipped_for_someone_behind_but_words_still_go() {
        let (raw, mut rx) = mpsc::unbounded_channel();
        let tx = ClientTx::new(raw);
        for _ in 0..STATUS_SKIP_QUEUED {
            tx.send(words()).unwrap();
        }
        tx.send(status()).unwrap();
        tx.send(words()).unwrap();
        let mut queued = Vec::new();
        while let Ok(m) = rx.try_recv() {
            queued.push(m);
        }
        assert_eq!(queued.len(), STATUS_SKIP_QUEUED + 1);
        assert!(queued.iter().all(|m| matches!(m, ToClient::Lines { .. })));
        for m in &queued {
            tx.taken(m);
        }
        tx.send(status()).unwrap();
        assert!(matches!(rx.try_recv(), Ok(ToClient::Status { .. })));
    }

    #[test]
    fn hooks_take_plain_text_or_slack_or_discord_json() {
        assert_eq!(hook_text(b"pizza is here \xF0\x9F\x8D\x95").as_deref(), Some("pizza is here 🍕"));
        assert_eq!(hook_text(br#"{"text":"the kettle has boiled"}"#).as_deref(), Some("the kettle has boiled"));
        assert_eq!(hook_text(br#"{"content":"hi from discord","username":"x"}"#).as_deref(), Some("hi from discord"));
        assert_eq!(hook_text(b"two\nlines\r\n  and   spaces").as_deref(), Some("two lines and spaces"));
        assert_eq!(hook_text(br#"{"check_name":"site","current_state":"DOWN","description":"timeout"}"#).as_deref(), Some("DOWN timeout"));
        assert_eq!(
            hook_text(br#"{"version":"1.0.0","organization":{"name":"acme","id":"o"},"project":{"name":"web","id":"p"},"revision":{"tag":null,"branch":{"name":"main"},"commit_message":"Fix the thing\n\nmore"},"workflow":{"id":"w1"},"pipeline":{"state":"done","result":"failed"}}"#).as_deref(),
            Some("failed [web main](https://acme.semaphoreci.com/workflows/w1): Fix the thing")
        );
        assert_eq!(hook_text(b"   "), None);
        assert_eq!(hook_text(br#"{"nope":1}"#), None);
        assert_eq!(hook_text(b"{not json"), None);
    }

    #[test]
    fn commands_go_at_a_gentle_pace() {
        let start = std::time::Instant::now();
        let mut pace = Pace::new(start);
        assert!((0..20).all(|_| pace.take(start)), "a rush of twenty is fine");
        assert!(!pace.take(start), "the twenty-first waits");
        assert!(pace.warn() && !pace.warn(), "said once, not every time");
        let later = start + std::time::Duration::from_millis(500);
        assert!(pace.take(later) && pace.take(later), "two more half a second later");
        assert!(!pace.take(later));
        assert!(pace.warn(), "running out again is said again");
    }

    #[tokio::test]
    async fn reads_a_post_with_its_body_across_packets() {
        use tokio::io::AsyncWriteExt;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = tokio::spawn(async move {
            let mut c = TcpStream::connect(addr).await.unwrap();
            c.write_all(b"POST /hook/abc HTTP/1.1\r\nHost: x\r\nContent-Length: 11\r\n\r\nhello").await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            c.write_all(b" world").await.unwrap();
            c
        });
        let (mut s, _) = listener.accept().await.unwrap();
        let Ok(req) = read_request(&mut s).await else { panic!("couldn't read it") };
        assert_eq!(req, Request { method: "POST".into(), path: "/hook/abc".into(), body: b"hello world".to_vec() });
        drop(client.await);
    }

    #[test]
    fn a_connection_hopelessly_behind_is_let_go() {
        let (raw, mut rx) = mpsc::unbounded_channel();
        let tx = ClientTx::new(raw);
        for _ in 0..MAX_QUEUED + 100 {
            tx.send(words()).unwrap();
        }
        assert!(tx.swamped());
        let mut n = 0;
        while rx.try_recv().is_ok() {
            n += 1;
        }
        assert_eq!(n, MAX_QUEUED, "nothing more is queued once it's swamped");
    }
}
