# world.txt ✨

A small shared island made of text. You can wander, chat, dig ponds, plant trees,
name places and make things. There are no battles, just vibes.

```
make install                       # builds and installs ~/.local/bin/world.txt
world.txt serve                    # opens the world on :4040, saves to world.db
world.txt play Mark                # terminal client
open http://localhost:4040         # browser client (works on iPads, plays sound)
```

Cargo won't put a `.` in a binary name, so `cargo build` makes `target/release/world`
and `make install` copies it into place as `world.txt`. `cargo run -- serve` works too.

One port serves both doors: websocket upgrades go to the game, and plain HTTP
gets the browser page (`web/index.html`, compiled into the binary).

## Inviting people from outside your network

Set a password, then open a tunnel:

```
WORLD_PASSWORD=froggy world.txt serve
cloudflared tunnel --url http://localhost:4040    # prints https://<random>.trycloudflare.com
```

- Browser: send them `https://<random>.trycloudflare.com/#pw=froggy`. The password
  is filled in for them, and the part after `#` never leaves their browser.
- Terminal: `world.txt play Sam --server wss://<random>.trycloudflare.com --password froggy`
  (or set `WORLD_SERVER` and `WORLD_PASSWORD` once and just run `world.txt play Sam`)

Without a password, anyone who can reach the port can join, so keep it to your
home network or Tailscale.

## In the world

- `n s e w ne nw se sw`, `look`, `who`
- `say hi` (or `'hi`), `:waves`, `shout`, `hum C E G`
- `dig`, `fill`, `plant tree|flowers|grass`, `raise`, `path`, `name The Willow Bank`
- `make a frog that croaks when it rains`, `push frog north`, `unmake frog`

Clouds drift across the island on the wind, and a whole day passes in 24 minutes.

## Layout

- `src/protocol.rs`: JSON messages with named colours, so any client can draw them
- `src/server/`: networking (`mod.rs`), the game loop and commands (`game.rs`),
  the island (`world.rs`, `terrain.rs`), weather and clock (`weather.rs`), SQLite (`db.rs`)
- `src/client.rs`: the ratatui terminal client
- `web/index.html`: the browser client
