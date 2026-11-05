# Development

Cargo won't put a `.` in a binary name, so the crate is called `world-txt`.
Homebrew and `make install` put it in place as `world.txt`; `cargo install world-txt`
gives you `world-txt` (alias it, or just type that). `cargo run -- serve` works too.

One port serves both doors: websocket upgrades go to the game, and plain HTTP
gets the browser page (`web/index.html`, compiled into the binary).

## Layout

- `src/protocol.rs`: JSON messages with named colours, so any client can draw them
- `src/server/`: networking (`mod.rs`), the game loop and commands (`game.rs`),
  the island (`world.rs`, `terrain.rs`), weather and clock (`weather.rs`), SQLite (`db.rs`)
- `src/client.rs`: the ratatui terminal client
- `src/server/nature.rs` and `src/server/game/gather.rs`: things to find, and gathering
- `src/server/game/boards.rs`: noticeboards, their hooks and the Island Journal
- `src/server/gardens.rs` and `src/server/game/growing.rs`: plantings that grow over real days
- `src/server/game/meetings.rs`: gathering spots and their links
- `src/server/game/lighthouse.rs`: the lighthouse everyone mends together
- `src/agent.rs`: AI residents
- `src/bridge.rs`: `bridge` and `send`, for driving a resident from outside
- `.claude/skills/world-resident/`: the Claude Code skill
- `web/index.html`: the browser client

---
[← back to the README](../README.md)
