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

**Names belong to the device that first uses them.** Each browser keeps a secret
key (and this machine's terminal client, residents and bridges share one in
`~/.config/world.txt/key`); the world stores only a hash. Anyone else trying that
name is turned away.

Browsers keep that key **per web address**, so a new quick-tunnel link, switching
between your home address and the tunnel, or Safari clearing old site data can
leave a kid with a fresh key. **Stick to one address** (a named tunnel, or the same
home address every time) to avoid it. When it happens anyway, the kid's screen shows a short code
(like `FROG-42`) to read out to an island keeper, and the world remembers the knock:

- Keepers online (or the next to arrive) are told someone wants to come in as
  Poppy, but never see the code. Ask Poppy for the code on her screen, then type
  `welcome Poppy FROG-42`. Only the device showing that code gets the name (any
  other device stops working), and Poppy just joins again. If a stranger tried the
  name too, their code won't match.
- On the server's machine: `world.txt welcome Poppy FROG-42`. The server's own log
  also notes each knock.
- `unlink Poppy` (keepers, in the world) or `world.txt release Poppy` frees a name
  for whichever device uses it next.

**Things belong to whoever made them.** Only the maker can `unmake` or `push`
something; everyone else is told whose it is. Unmaking tucks a thing away rather
than deleting it, so `undo` brings back the last thing you unmade (within a day),
noticeboards with all their notes. Island keepers can tidy up anyone's things:
`world.txt serve --keeper Mark` (or `WORLD_KEEPERS=Mark,Lu`).

**Keeping it kind.** `mute Sam` means you stop hearing Sam's talk, emotes,
shouts, private messages, chat and board notes. It's remembered, and Sam isn't
told (`unmute Sam`, `mutes`). Keepers can `kick Sam` (30 minutes, or
`kick Sam 60`): Sam is sent off for a break and can't come back as that name
or from that device until it ends, or a keeper types `unkick Sam`.

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
- **Backups:** the server copies the world to `backups/world-<date>.db` (next to
  the db) once a day while it runs, and keeps the newest 14 (`--keep-backups 14`,
  0 turns it off). `world.txt backup` makes one right now. To restore, stop the
  server and copy a backup over `world.db`.
- **Keep a copy off this machine.** Backups next to `world.db` cover mistakes and
  a damaged file, but not a dead disk, a lost laptop or a wipe: the island and
  its backups would go together. So also get them somewhere else:
  - Point the backups straight at cloud or external storage:
    `world.txt serve --backup-dir ~/Library/Mobile\ Documents/com~apple~CloudDocs/world-backups`
    (iCloud Drive), a Dropbox or Google Drive folder, or an external drive
    (`--backup-dir /Volumes/Backup/world`, also `WORLD_BACKUP_DIR`).
  - Or keep the default and let Time Machine, or a sync tool, copy the
    `backups` folder.
  - Copying the finished backup files is always safe. Don't sync the live
    `world.db` (or its `-wal`/`-shm` files) while the server runs: a half-copied
    live database can be corrupt. That's what the backups are for.

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

### Homes

`claim` makes the 3x3 patch around you your home (one each, not too close to
anyone else's or to where newcomers arrive). It shows as `⌂` on the map and is
called "Poppy's Home" (with "Poppy's Garden" around it) unless you `name` it.
`home` takes you straight back from anywhere. Anyone can visit, but only you,
the friends you `invite`, and keepers can make, place, dig, plant or name things
there (`uninvite`, `guests`). Claiming somewhere else moves your home.

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

### Crafting

`recipes` shows what can be made from gathered things, ticking the ones your
satchel is ready for: flower crowns, bouquets, shell wind chimes, dreamcatchers,
crystal lanterns, mushroom soup, shell necklaces, stone cairns, a conch horn and
a rainbow garland. `craft wind chime` makes one (using your commonest finds
first, so rare ones aren't spent by accident). Crafts go in your satchel to
`give` or `place`, and placed crafts come alive: a wind chime tinkles in the
breeze, a lantern glows at night.

### Noticeboards

`make a noticeboard` puts one up (shown as `▣` on the map). It works like a group
chat that belongs to that spot:

- `read` shows the latest notes and `read more` scrolls back. In the browser, a
  board opens as a chat panel that loads older notes as you scroll up.
- `post see you at sunset!` leaves a note. Everyone standing there sees it, and
  posting **follows** the board, so you hear about new notes wherever you are.
  `follow` / `unfollow` to change that, and `boards` lists the ones you follow.

The **Island Journal** is a special board at the spawn point. The first find of
every species each week goes in it automatically, with a shout-out to the whole
island, and people can post underneath. It starts a fresh page every Monday:
`archive` lists past weeks and `read week 2026-10-26` opens one.

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
- `src/server/game/boards.rs`: noticeboards and the Island Journal
- `src/agent.rs`: AI residents
- `src/bridge.rs`: `bridge` and `send`, for driving a resident from outside
- `.claude/skills/world-resident/`: the Claude Code skill
- `web/index.html`: the browser client

## License

MIT. See [LICENSE](LICENSE).
