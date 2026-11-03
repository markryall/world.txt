# Changelog

## 0.2.1 (2026-11-03)

- `link me` gives you a magic link for your own new device (phone, laptop,
  tablet), so you don't need a keeper for it. Keepers can still `link Poppy`.
- Arriving again takes over from your old connection: a phone or tab that's
  gone to sleep no longer locks you out with "already here".

## 0.2.0 (2026-11-03)

Music, cooking, and a world that stays up on its own.

### Music on the island
- **Gramophones:** `make a gramophone`, stand by it and `play music`, then choose
  songs from your computer. Everyone nearby hears them, loudest beside it and
  fading over five steps. The songs never leave your computer except as the live
  stream: the world only passes it on to people in earshot.
- **Jukeboxes:** a gramophone everyone can add to. `add songs`, and it plays one
  song per person, taking turns, each streamed from its owner's browser when
  their turn comes. `jukebox` shows the queue; `skip` and `unqueue`.
- **Radios:** `make a radio` and `tune radio <station>` plays one of the
  island's radio stations for everyone nearby, straight from the station.
  Island keepers choose the stations (`station add`, `station remove`), and
  anyone can see them (`stations`) and tune in. Radios say what's playing when
  the stream does, or what show is on air from the station's programme guide.
  `world.txt add-stations` adds a recommended list of community and
  independent stations, and SomaFM (see `stations.txt`).
- Sound makers must be more than ten steps apart, and you only ever hear the
  nearest one: never two songs at once.

### Cooking
- Make a campfire or a clay oven anywhere, then `cook mint tea` from things
  you've gathered. While it cooks, its smell drifts to everyone nearby (as it
  goes on, halfway, and when it's done). Then `eat` it, or `serve` it to everyone
  standing with you.
- Six dishes to start: mint tea, toasted acorns, grilled fish, mushroom stew,
  berry crumble and honey cake. Wild mint, thyme and honeycomb turn up as you
  wander, and dishes you cook go in your journal.

### Hosting
- **Fly.io:** a `Dockerfile`, a settings template and `deploy/fly-deploy` run
  each world on one small always-on machine, with your own settings kept out of
  git. Bring an existing world across by uploading it as `import.db`. Several
  worlds side by side, each with its own app, volume and address;
  `deploy/fly-deploy all` deploys them all and says how long each took.
- **macOS services:** `world.txt service install` keeps a world (and its
  Cloudflare tunnel) running from login, restarting it if it falls over; several
  worlds can run side by side with `--name` and `--port`.
- `--db` everywhere also reads `WORLD_DB`.
- `world.txt rename "Willow Bank"` gives a world a proper name, shown on the join
  screen and to other worlds.

### Invites and accounts
- A magic link opens on an **Enter the world** button and is only used when it's
  clicked, so a message's link preview can't use it up first.
- A used invite is cleared from the address bar, so a refresh doesn't turn you
  away.
- `invites` lists recent magic links as waiting, used or expired, and `revoke`
  cancels someone's unused ones (in the world, or `world.txt invites` /
  `world.txt revoke`).
- `fold` moves an old account from before nicknames into someone's account.
  Names that could mean more than one account ask for the username instead, and
  `invite` refuses a nickname that's taken.

### In the browser
- The command box keeps the cursor: click anywhere, or just start typing.
- Commands you've typed are remembered across visits; ↑/↓ steps through them and
  **Ctrl-R** searches them like a shell.
- Hover over a line to see when it happened.
- `help` comes a topic at a time (`help music`, `help cooking`, `help all`).

### Quieter
- Scenery never repeats itself back to back, and goes quiet for someone who's
  stepped away.
- A made thing (say, a croaking frog) does its thing for each person at most
  every three minutes.
- "No battles, just bonhomie."

## 0.1.0 (2026-11-02)

The first release: a cozy multiplayer text world, played in the browser or a
terminal.

- Wander a shared island (and three more by boat), with weather, a day that
  passes in 24 minutes, weekly seasons and Saturday festivals.
- Chat nearby, message anyone (kept as mail if they're away), shout, hum and
  jam, and walk to friends with `join`.
- Shape the land: dig, fill, plant, raise, make paths and name places.
- Make things that come alive (`make a frog that croaks when it rains`), music
  boxes that learn tunes, and noticeboards, including the weekly Island Journal.
- Gather flowers, shells, mushrooms, feathers, stones, berries and fish, fill a
  journal, craft keepsakes, befriend an animal companion, and claim a home.
- Invite-only accounts with magic links and device tokens, island keepers who
  look after things (invite, link, kick, ban, promote, events), mutes for
  everyone, and nightly backups.
- Visit other worlds with signed passports, and let keepers trust friendly ones.
- AI island residents, and a bridge for driving one from Claude Code.
- Install from crates.io (`cargo install world-txt`) or Homebrew.
