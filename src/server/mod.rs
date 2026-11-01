//! Networking. Each connection is a small task that shuttles messages
//! between its websocket and the single game task that owns the world.

mod db;
mod game;
mod rng;
mod terrain;
mod weather;
mod world;

use crate::protocol::{ToClient, ToServer};
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;

pub type ClientTx = UnboundedSender<ToClient>;

pub enum Event {
    Join { id: u64, name: String, tx: ClientTx },
    Cmd { id: u64, text: String },
    Leave { id: u64 },
}

pub async fn run(addr: &str, db_path: &str, seed: u64) -> anyhow::Result<()> {
    let db = db::Db::open(db_path)?;
    let world = world::World::load_or_create(db, seed)?;
    let (events_tx, events_rx) = mpsc::unbounded_channel();
    tokio::spawn(game::run(world, events_rx));

    let listener = TcpListener::bind(addr).await?;
    println!("🌍 world.txt is open: browser http://{addr}  ·  terminal ws://{addr}  (db: {db_path})");
    let mut next_id = 1u64;
    loop {
        let (stream, peer) = listener.accept().await?;
        let id = next_id;
        next_id += 1;
        let events = events_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = connection(id, stream, events).await {
                eprintln!("connection {peer}: {e}");
            }
        });
    }
}

async fn connection(
    id: u64,
    stream: TcpStream,
    events: UnboundedSender<Event>,
) -> anyhow::Result<()> {
    if !is_websocket(&stream).await? {
        return serve_page(stream).await;
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
            ToServer::Hello { name } if !joined => {
                joined = true;
                events.send(Event::Join { id, name, tx: tx.clone() })?;
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

async fn serve_page(mut stream: TcpStream) -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req.split_whitespace().nth(1).unwrap_or("/");
    let (status, ctype, body) = match path {
        "/" | "/index.html" => ("200 OK", "text/html; charset=utf-8", PAGE),
        _ => ("404 Not Found", "text/plain; charset=utf-8", "nothing here, babe\n"),
    };
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}
