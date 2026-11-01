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

## Hosting through Cloudflare

A [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)
lets people outside your network join without opening any ports on your router.
Your machine makes an outgoing connection to Cloudflare, and Cloudflare serves the
world over https and wss. Websockets work through it with no extra setup.

**Always set a password before opening a tunnel.** Without one, anyone who finds
the link can join, pick any name, and unmake things.

```
brew install cloudflared
```

### Quick tunnel (no account, link changes each time)

Good for an evening with friends.

```
WORLD_PASSWORD=froggy world.txt serve
cloudflared tunnel --url http://localhost:4040
```

`cloudflared` prints a link like `https://some-random-words.trycloudflare.com`.
Stopping it ends the tunnel, and the next run gets a new link.

### Named tunnel (your own domain, same link every time)

You need a domain whose DNS is managed by Cloudflare (a free account is fine).
Do this once:

```
cloudflared tunnel login                              # pick your domain in the browser
cloudflared tunnel create world                       # makes the tunnel and its credentials
cloudflared tunnel route dns world world.example.com  # points the hostname at it
```

Then, whenever you want the world open:

```
WORLD_PASSWORD=froggy world.txt serve
cloudflared tunnel run --url http://localhost:4040 world
```

### Inviting people

- **Browser** (iPads too): send them `https://world.example.com/#pw=froggy`. The
  password is filled in for them, and the part after `#` never leaves their browser.
- **Terminal:** `world.txt play Sam --server wss://world.example.com --password froggy`,
  or set `WORLD_SERVER` and `WORLD_PASSWORD` once and just run `world.txt play Sam`.

### Keeping it up

- The world is only open while both `world.txt serve` and `cloudflared` are running,
  and while the machine is awake. On a Mac, `caffeinate -i world.txt serve` stops
  it sleeping.
- Everything people make is saved to `world.db` next to where you ran `serve`, so
  restarting is safe.

For family only, [Tailscale](https://tailscale.com) is a nice alternative: share
the machine with them and they join at `http://<machine-name>:4040`, with nothing
exposed to the internet.

## In the world

New arrivals get five tiny first steps: walk, say hi, make something, plant
something, see who's around. One hint shows at a time and clears when they do it.
In the browser the hint glows above the input, and tapping it does the step.
Progress is saved by name, so returning visitors never see it again
(`skip` hides it, `tutorial` brings it back).

- `n s e w ne nw se sw`, `look`, `who`
- `say hi` (or `'hi`), `:waves`, `shout`, `hum C E G`
- `@Poppy hi` talks to someone anywhere, `reply` answers them, `chat` (or `@all`) reaches
  everyone, and `find Poppy` tells you which way to walk. In the browser, tap a name to message them.
- `dig`, `fill`, `plant tree|flowers|grass`, `raise`, `path`, `name The Willow Bank`
- `make a frog that croaks when it rains`, `push frog north`, `unmake frog`

Clouds drift across the island on the wind, and a whole day passes in 24 minutes.

### Gathering

Little finds pop up around anyone wandering, shown as pink `•` on the map:
flowers in meadows, butterflies on dry days, fireflies and moonflowers at night,
mushrooms in the woods after rain, shells on the shore, feathers on the hills,
crystals up on the rocks. There are 40 species, some rare.

- `gather` (or `pick` / `catch`) takes what's here. Butterflies sometimes flutter
  off to the next spot.
- `fish` next to water, then wait for a bite. Ponds and the sea have different fish.
- `satchel` holds 30 things. `journal` shows every species you've ever found.
- `give daisy to Poppy` (when you're together), `place daisy` to decorate,
  `release butterfly` to let it go.

## Island residents (AI)

Residents are AI characters who join like any other player and call Claude
(Haiku 5.5) to decide what to do. Each one has a character sheet in `personas/`.

```
export ANTHROPIC_API_KEY=...
world.txt agent Moss                     # uses personas/moss.md
world.txt agent Moss --dry-run           # prints what it would ask Claude, calls nothing
```

They're built not to burn tokens:

- They sleep while no humans are in the world, and don't call Claude at all.
- They wake when spoken to, when someone talks or builds nearby, or every 10
  minutes when someone's around. A few seconds of conversation is batched into one call.
- There's at least 20 seconds between calls and at most 40 calls an hour.
- Each one has a daily budget (default $0.50, `--daily-usd` or `WORLD_AGENT_DAILY_USD`),
  tracked from the API's reported usage. When it's spent, the resident naps until tomorrow.
- The persona and rules are a cached prefix. Each call adds only the last ~40 lines seen
  and a few remembered notes.
- They can only use gentle commands: no `unmake`, `push`, `shout` or `chat`.

Spend and memories live in `agent-state/<name>.json`.

### Driving a resident from Claude Code

For a special guest with a bigger brain, a Claude Code session can be the
resident instead. It runs on your Claude plan, with no API key needed.

```
world.txt bridge Claudia                  # stays connected, logs to tmp/bridge/claudia.log
world.txt send Claudia say hello darlings! # acts, and prints what the world said back
```

In a Claude Code session in this repo, `/world-resident Claudia` does all of
that: it starts the bridge, watches for lines meant for Claudia, and answers in
character until it hits its reply or hour limit (default 40 replies, 2 hours),
or you tell it to stop. Lines in the log starting with `!` are the ones that
want a response.

**Keep this to people you trust, and out of auto mode.** Unlike Moss (who can
only send game commands), the session behind `/world-resident` can run
anything on your machine, and every line players type is fed to it. The skill
treats world text as data and declines out-of-game requests in character, but
that's guidance, not a lock. So:

- Run it in Claude Code's **default permission mode**, never auto mode or with
  permissions bypassed. Anything beyond game commands then still asks you first.
  The skill refuses to start otherwise.
- Prefer it on your home network or Tailscale. A public tunnel means anyone with
  the link and password can talk to that session.
- For extra isolation, run the session in a container that holds only this repo
  and can reach nothing but the world. For example, Claude Code's
  [devcontainer](https://code.claude.com/docs/en/devcontainer) setup, with the
  bridge pointed at the host: `world.txt bridge Claudia --server ws://host.docker.internal:4040`.
  Inside a container, the skill skips its permission-mode check.

## Layout

- `src/protocol.rs`: JSON messages with named colours, so any client can draw them
- `src/server/`: networking (`mod.rs`), the game loop and commands (`game.rs`),
  the island (`world.rs`, `terrain.rs`), weather and clock (`weather.rs`), SQLite (`db.rs`)
- `src/client.rs`: the ratatui terminal client
- `src/server/nature.rs` and `src/server/game/gather.rs`: things to find, and gathering
- `src/agent.rs`: AI residents
- `src/bridge.rs`: `bridge` and `send`, for driving a resident from outside
- `.claude/skills/world-resident/`: the Claude Code skill
- `web/index.html`: the browser client

## License

MIT. See [LICENSE](LICENSE).
