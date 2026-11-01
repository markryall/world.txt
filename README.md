# world.txt ✨

A small shared island made of text. You can wander, chat, dig ponds, plant trees,
name places and make things. There are no battles, just vibes.

```
cargo run -- serve                 # opens the world on :4040, saves to world.db
cargo run -- play Mark             # terminal client
open http://localhost:4040         # browser client (works on iPads, plays sound)
```

One port serves both doors: websocket upgrades go to the game, and plain HTTP
gets the browser page (`web/index.html`, compiled into the binary).

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
