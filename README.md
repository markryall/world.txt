# world.txt ✨

A small shared island made of text. You can wander, chat, dig ponds, plant trees,
name places and make things. There are no battles, just bonhomie.

```
brew install markryall/tap/world-txt  # or: make install (builds ~/.local/bin/world.txt)
world.txt serve                      # opens the world on :4040, saves to world.db
world.txt invite Mark --keeper       # your account, and its magic link: open it!
world.txt play --invite <link>       # or the terminal client (just `world.txt play` after)
```

**The world is invite-only.** Every account is made by a keeper (or on the
server's machine with `world.txt invite`), which prints a **magic link**. Opening
it signs that device in for good: the link is used up, and the browser (or
terminal) keeps a device token instead, so next time it just opens. Links work
once and expire after 7 days. For a new device of your own, type `link me` in
the world and open the link it gives you there. A keeper can also make one for
someone (`link Poppy` in the world, or `world.txt link Poppy`). Other devices
keep working. In the world, keepers type `invite Poppy` to make an account for
someone new. Pass `--url https://your.address` (or `WORLD_URL`) to `serve` and
`invite` so links come out whole; otherwise the browser fills in its own address.
Browsers keep the device token per web address, so stick to one address (a named
tunnel, or the same home address). A link opens on an **Enter the world** button
and is only used when that's clicked, so a message's link preview (iMessage, a
mail scanner) can't use it up first.

**Keepers** look after the island. In the world they can `invite` and `link`,
`promote Lu` / `demote Lu` (the last keeper can't step down), `kick Sam` for a
break or `ban Sam` for good (`unban`), `signout Sam` to sign out all of
someone's devices (a lost iPad: then `link Sam` for a fresh start), and `users`
to see everyone with their devices and when they were last seen. `invites` shows
recent magic links as waiting, used or expired, and `revoke Sam` cancels Sam's
unused ones (a link sent to the wrong person). Old accounts from before nicknames
can be folded into someone's account with `fold Poppy into sunny-otter-42`
(usernames, from `users`): their satchel, journal, things, notes and mail move
across. When a name could mean more than one account, commands ask for the
username instead. On the server's machine, `world.txt promote Lu` always works,
as do `world.txt invites` and `world.txt revoke Sam`.

Cargo won't put a `.` in a binary name, so the crate is called `world-txt`.
Homebrew and `make install` put it in place as `world.txt`; `cargo install world-txt`
gives you `world-txt` (alias it, or just type that). `cargo run -- serve` works too.

**Everyone has a username and a nickname.** Your username (like
`sunny-otter-42`) is made up for you the first time you arrive and never changes;
everything of yours (satchel, journal, home, things, notes, mail) belongs to it.
Your nickname is what people see, and `nickname Pip` changes it any time. `who`
shows both, arrivals and renames show both, and `@`, `mute`, `kick`, `link` and
friends accept either.

**Things belong to whoever made them.** Only the maker can `unmake` or `push`
something; everyone else is told whose it is. Unmaking tucks a thing away rather
than deleting it, so `undo` brings back the last thing you unmade (within a day),
noticeboards with all their notes. Island keepers can tidy up anyone's things.

**Keeping it kind.** `mute Sam` means you stop hearing Sam's talk, emotes,
shouts, private messages, chat and board notes. It's remembered, and Sam isn't
told (`unmute Sam`, `mutes`). Keepers can `kick Sam` (30 minutes, or
`kick Sam 60`): Sam is sent off for a break and can't come back until it ends,
or a keeper types `unkick Sam`. Since only invited people get in, a kick or ban
can't be dodged with a new name.

One port serves both doors: websocket upgrades go to the game, and plain HTTP
gets the browser page (`web/index.html`, compiled into the binary).

## Hosting through Cloudflare

A [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)
lets people outside your network join without opening any ports on your router.
Your machine makes an outgoing connection to Cloudflare, and Cloudflare serves the
world over https and wss. Websockets work through it with no extra setup.

The world is invite-only, so the tunnel's address on its own doesn't let anyone
in: only the magic links you hand out do.

```
brew install cloudflared
```

### Quick tunnel (no account, link changes each time)

Good for an evening with friends.

```
world.txt serve
cloudflared tunnel --url http://localhost:4040
```

`cloudflared` prints a link like `https://some-random-words.trycloudflare.com`.
Stopping it ends the tunnel, and the next run gets a new link, which means
everyone's browser needs a fresh magic link too (browsers keep the device token
per address). For anything regular, use a named tunnel.

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
world.txt serve --url https://world.example.com
cloudflared tunnel run --url http://localhost:4040 world
```

### Your own domain, step by step

1. **Put the domain on Cloudflare.** If you bought it somewhere else, add it as a
   site in the Cloudflare dashboard (the free plan is fine), then go to your
   registrar and change the domain's **nameservers** to the two Cloudflare gives
   you. The switch can take from minutes to a day; Cloudflare emails you when the
   domain is active.

2. **Pick the address** the world will live at, either the bare domain
   (`example.com`) or a subdomain (`world.example.com`). Both work. Settle on one
   and keep it: it's in every magic link, browsers keep their sign-in per address,
   and it's your world's identity when visiting other worlds (changing it means
   fresh links for everyone, and friendly worlds would need to trust you again).

3. **Make the tunnel and point the address at it** (once):

   ```
   cloudflared tunnel login                         # choose the domain in the browser
   cloudflared tunnel create world
   cloudflared tunnel route dns world example.com   # or world.example.com
   ```

4. **Tell world.txt its address,** so `serve`, `invite` and `link` all use it. Put
   this in your shell's startup file (e.g. `~/.zshrc`, or `set -gx` in fish):

   ```
   export WORLD_URL=https://example.com
   ```

5. **Open the world:**

   ```
   world.txt serve
   cloudflared tunnel run --url http://localhost:4040 world
   ```

   Check it with `curl https://example.com/.well-known/world.txt`: it should show
   your world's name and address.

   Give it a proper name too, if you like: `world.txt rename "Willow Bank"`.
   Visitors and other worlds see it; the address stays the same.

6. **Make yourself a keeper** and open the link it prints:

   ```
   world.txt invite YourName --keeper
   ```

7. **Keep it up without a terminal** (macOS): from the world's folder,

   ```
   world.txt service install --url https://example.com --tunnel world
   ```

   runs the world and its tunnel in the background, starts them at every login
   and brings them back if they fall over. `world.txt service status` checks on
   them, logs are in `~/Library/Logs/world.txt/`, and `world.txt service uninstall`
   stops them. Run `install` again after upgrading world.txt.

   **More than one world?** Give each its own folder, address, tunnel, name and
   port:

   ```
   cloudflared tunnel create garden
   cloudflared tunnel route dns garden garden.example.com
   world.txt service install --db ~/worlds/garden/world.db --url https://garden.example.com \
     --tunnel garden --name garden --port 4041
   ```

   `service status` lists every world, and `service uninstall --name garden` (or
   `--all`) stops them. Each world's tunnel gets its own settings, so they never
   pick up `~/.cloudflared/config.yml` or each other's. (On Linux, a systemd
   unit running `world.txt serve` and Cloudflare's own `cloudflared service install`
   do the same job.)

### Inviting people

- Make their account and link: `invite Sam` in the world (keepers), or
  `world.txt invite Sam --url https://world.example.com` on the server's machine.
- **Browser** (iPads too): send them the link. Opening it signs that device in.
- **Terminal:** `world.txt play --server wss://world.example.com --invite <link>`
  the first time (set `WORLD_SERVER` to skip `--server`), then just `world.txt play`.

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

## Hosting on Fly.io

A laptop world goes dark whenever the laptop sleeps. [Fly.io](https://fly.io)
keeps it up on one small always-on machine for a couple of dollars a month,
with the world on a volume. The repo has what it needs: a `Dockerfile`,
`deploy/fly-start.sh`, a settings template in `deploy/fly/example.toml` and
`deploy/fly-deploy` to deploy with.

Each world is its own Fly app, with its own settings file in `deploy/fly/`
(e.g. `deploy/fly/home.toml`). Those files are ignored by git, so your app
names and addresses stay on your machine.

### Setting up a world (once per world)

1. Install `flyctl` (`brew install flyctl`) and `fly auth login`.
2. Copy the template and fill it in: `app` (a name of your own, unique on Fly),
   `primary_region` (one near your people), `WORLD_URL` (the world's address)
   and `TZ` (the world's own clock, e.g. `Australia/Sydney`: festivals and the
   Journal's weeks follow it, as do times for anyone whose device hasn't said
   its timezone; Fly machines are UTC otherwise).

   ```
   cp deploy/fly/example.toml deploy/fly/home.toml
   ```

3. Make the app and its volume, in the same region, then deploy:

   ```
   fly apps create <app>                 # add --org <org> to bill it to an organisation
   fly volumes create world_data --size 1 --region <region> --app <app>
   deploy/fly-deploy home
   ```

   That starts a fresh, empty world. `curl https://<app>.fly.dev/.well-known/world.txt`
   to check it's up.
4. **Its address:** `fly certs add world.example.com --app <app>`, then in your
   DNS point `world.example.com` at `<app>.fly.dev` with a CNAME (on Cloudflare,
   set it to **DNS only**, the grey cloud, and remove any tunnel record for that
   name). `fly certs show world.example.com --app <app>` says when it's ready.
5. **Make yourself a keeper:** `fly ssh console --app <app> -C "world.txt invite YourName --keeper"`

**More worlds** are the same steps with another settings file, app, volume and
address (say `deploy/fly/garden.toml` for `garden.example.com`). Each has its own
machine, volume, identity, accounts and backups, so they never share anything,
and a deploy only bounces the world it's for.

### Bringing an existing world across

People keep their sign-ins as long as the address stays the same, because
their device tokens live in `world.db`, along with the world's identity key.

1. Stop the old server (and its tunnel), so nothing changes after the copy.
2. Copy the world safely: `sqlite3 world.db ".backup /tmp/world-move.db"`
3. Upload it as `import.db`, then restart. On start, `fly-start.sh` moves it
   into place before the server opens anything, and keeps the world it replaced
   in `/data/replaced/`:

   ```
   fly ssh sftp shell --app <app>
   » put /tmp/world-move.db /data/import.db
   fly machine restart --app <app>
   ```

4. Check `curl https://world.example.com/.well-known/world.txt` shows the same
   `public_key` as before.

### Deploying changes

```
deploy/fly-deploy home      # one world
deploy/fly-deploy all       # every world in deploy/fly/
```

Each deploy builds the image (on Fly's own builders, so Docker isn't needed
locally), pushes it to Fly's registry and swaps that world's machine over. With
one machine and a volume, that's a few seconds' gap. The world tells everyone
it's being tidied up, and browsers wait and come straight back in on their own,
keeping what was on screen (the terminal client asks you to run `play` again).
Gramophone songs stop and need playing again, so it's still kindest to deploy
when the island is quiet.

### Keeper chores on Fly

Commands run on the machine find the world on their own (`WORLD_DB` and
`WORLD_URL` are set there):

```
fly ssh console --app <app> -C "world.txt invite Sam"
fly ssh console --app <app> -C "world.txt invites"
fly ssh console --app <app> -C "world.txt rename 'Willow Bank'"   # then fly machine restart
fly logs --app <app>                                            # what the world is saying
```

**Backups** land in `/data/backups/`, on the same volume as the world, and Fly
also snapshots volumes daily. To keep a copy off Fly too, pull one down now and
then: `fly ssh sftp get /data/backups/world-<date>.db --app <app>`.

## In the world

New arrivals get five tiny first steps: walk, say hi, make something, plant
something, see who's around. One hint shows at a time and clears when they do it.
In the browser the hint glows above the input, and tapping it does the step.
Progress is saved by name, so returning visitors never see it again
(`skip` hides it, `tutorial` brings it back).

- `n s e w ne nw se sw`, `look`, `who`, and `map` for the whole island at half
  scale: you, friends, homes, boards, and numbered named places
- `say hi` (or `'hi`), `:waves`, `shout`, `hum C E G`
- `@Poppy hi` talks to someone anywhere (if they're away it's kept as mail and
  shown when they next arrive), `reply` answers them, `chat` (or `@all`) reaches
  everyone, `find Poppy` tells you which way to walk, and `join Poppy` walks you
  there automatically (around the water, following if they move; `stop` to stop).
  In the browser, tap a name to message them.
- `dig`, `fill`, `plant birch|bluebells|grass` (see Gardens), `raise`, `path`, `name The Willow Bank`
- `make a frog that croaks when it rains`, `push frog north`, `unmake frog`
- `make a music box`, then `teach C E G E C` (only its maker can) and `play`; it also
  plays now and then when people are near. Two people humming at the same spot
  are told they're making music together. In the browser, tunes play as real notes.
- `make a gramophone`, stand by it and `play music`, then choose songs from your
  computer: everyone nearby hears them, loudest beside it and fading over five
  steps (`stop music` to stop). `make a jukebox` is a gramophone everyone can add
  to: `add songs` picks songs from your computer (only their titles go to the
  world), and it plays one song per person, taking turns, each streamed from its
  owner's browser when their turn comes (`jukebox` shows the queue, `skip`,
  `unqueue`). Keep your tab open until your turn. `make a radio` and `tune radio <station>` plays
  one of the island's radio stations the same way (`radio off`); `stations`
  lists them, and part of a name will do. Island keepers choose the stations:
  `station add Night Owl FM https://… [guide https://…]` and `station remove`.
  `world.txt add-stations` (on the server's machine) starts a world off with a
  recommended list from `stations.txt`: community radio, SomaFM and more.
  Radios say what's playing when the stream does, or, with a guide (AirNet's
  format, used by many community stations), what show is on air. Sound makers must be more
  than ten steps apart, and you only ever hear the nearest, never two at once.
  Browser only, with the sound on; playing a gramophone needs a browser with
  WebCodecs (Chrome, Edge, recent Safari and Firefox).

Clouds drift across the island on the wind, and a whole day passes in 24 minutes.

### Homes

`claim` makes the 3x3 patch around you your home (one each, not too close to
anyone else's or to where newcomers arrive). It shows as `⌂` on the map and is
called "Poppy's Home" (with "Poppy's Garden" around it) unless you `name` it.
`home` takes you straight back from anywhere. Anyone can visit, but only you,
the friends you `share` it with, and keepers can make, place, dig, plant or name
things there (`unshare`, `guests`). Claiming somewhere else moves your home.

Each spot of your home can have your own words, which anyone who looks there
sees in place of the land's: `describe A messy little den, records stacked
everywhere and fairy lights strung over the jukebox.` (up to 280 characters;
`describe` on its own puts it back).

### Gardens

What you plant grows slowly, over real time. A tree starts as a sapling, is a
young tree after a day, a tall one after three and an old, mossy one after ten.
Flowers come up as shoots, are in bud after a couple of hours and bloom after
twelve. Nothing wilts and nothing needs watering. `plant oak` (or `birch`,
`willow`, `apple tree`, `cherry tree`, `pine`, `rowan`) or `plant bluebells` (or
`poppies`, `sunflowers`, `lavender`, `foxgloves`); `plant tree` and `plant
flowers` let the island choose, with willows by the water. Looking at the spot
says how it's coming along, who planted it and when, with the season's touch
(apple blossom in spring, apples in autumn). Young plantings show lighter on the
maps. When something you planted grows, you're told: straight away if you're
around, or under "While you were away" when you next arrive. Digging or
reshaping the spot clears it.

### Islands and boats

The world is an archipelago of four islands, each in its own patch of sea:
**Home Island** (where everyone arrives), **Driftwood Isle** (long beaches, with
pearls, sand dollars and driftwood), **Mossy Holm** (deep woods, with giant
puffballs, glow-worms and fern fronds) and **Ember Rock** (warm stone heights,
with obsidian, fire opals and sun lizards). Each island has a dock (`⚓` on the
map) with a rowing boat: `go dock` walks you there, `sail` lists where you can
go, and `sail Driftwood` rows you across. `map` shows the island you're on,
`find` tells you which island a friend is on, and homes can be claimed on any
island. Worlds from before the archipelago keep their island as Home Island and
grow the other three the first time they start.

### The lighthouse

An old ruined lighthouse stands on Driftwood Isle's outermost point (Lighthouse
Point), and everyone mends it together, a stage at a time: a driftwood door,
stairs mended with smooth pebbles, a lens of quartz crystals, and then a light
kindled with something that glows (fire opals, glowcaps or glow-worms). Standing
there, `mend lighthouse` gives it what it needs next from your satchel. Looking at
it says what it's missing and roughly how far along it is, only ever in words,
and thanks everyone who has helped. Each finished stage is shouted to the whole
world and written in the Island Journal. Once it's lit, its beam sweeps across
the water at night, and it shows as `☼` on the map.

### Visiting other worlds

Worlds can visit each other. Each world has a signing key, and (with
`serve --url https://your.address`) publishes its public key at
`/.well-known/world.txt`.

- At a dock, `sail to other.world.address` stamps a passport (signed by your
  world, good for 10 minutes and one use) and the browser rows you there.
- The other world checks the passport against your home world's key. Then you
  wait on its dock while its keepers are asked: `admit Poppy` lets you in on the
  spot, `turn away Poppy` sends you home. Once admitted, you're in for good: your
  browser gets a token for that world, so next time you walk straight in.
- Away from home you're shown as `Poppy (sunny-otter-42@your.address)`, with a
  fresh satchel and journal there (yours stay at home). `go dock` then
  `sail home` takes you back.
- Keepers see who's waiting when they arrive, and `visitors` lists everyone
  who's sailed in. Bans, mutes and kicks work on visitors too.
- Keepers can `trust their.address` so a friendly world's people come straight
  in without waiting (anyone already waiting from there is let in at once),
  `untrust` it again, and see the list with `worlds`.

Visiting happens in the browser (the terminal client points you there). A world
without `--url` can't send or receive visitors.

### Seasons, festivals and events

The island's seasons turn every Monday (a whole year in a month): blossom in
spring, dragonflies in summer, falling leaves and acorns in autumn, frost and
snow in winter, each with its own finds, descriptions and weather. Every
Saturday is the season's festival (the Blossom Picnic, Midsummer Beach Night,
Lantern Night, Snow Lights), with festival-only keepsakes hidden around the
island that glow once placed. (`WORLD_SEASON=autumn` and `WORLD_FESTIVAL=1`
force them, for testing or a treat.)

Keepers plan events where they're standing: `event sat 4pm Beach party` (or
`tomorrow 10:30am`, `noon`...). Times are your own: browsers (and the terminal
client) tell the world which timezone they're in, so 2pm means 2pm where you
are, and everyone sees event times in theirs. Notes everyone reads alike (the
Island Journal, a board) name the zone: "2:00pm AEST". The side panel shows your real
date and time (🕰 … your time) under the island's own clock, and `events` starts
with it too. In the browser, everyone else out in the world is listed at the foot of
the side panel with their local time (tap a name to talk to them, and whoever's
on your spot is marked "here"); `who` shows what time it is for everyone online
("3:12am Fri in London"), and `time Poppy` tells you Poppy's, from where they
last were if they're away. It goes in the Island Journal, everyone hears a
reminder ten minutes before and a shout when it starts, `events` lists what's
coming up, and `go event` walks you there. `cancel event 1` for its host or a keeper.

Events can also belong to just one place, so a little get-together doesn't ping
the whole island: `event here fri 4pm Tea party`. Anyone can plan one. Only its host,
people who've said `going 2` (the number from `events`; `not going 2` drops
out), followers of a noticeboard on that spot, and anyone within ten steps hear
it announced, reminded and starting. It's posted on that noticeboard rather
than the Island Journal, and `events` marks it 📍 with who's going.
Events can repeat: `event here fridays 5pm Frog choir` (also `weekdays`,
`weekends`, `daily`, or `every mon wed`). It stays at 5pm in the planner's
time, even across daylight saving, moves on to the next one an hour after each
starts, and `events` shows it as "🔁 every fri · next Fri 6 Nov, 5:00pm". Who's
going, the board's followers and its links carry over; `cancel event 1` stops
the series.
`event here sun 10am Pancakes with Poppy, Sam and Lu` signs them up as going
too: they're told straight away, or find a note waiting when they next arrive.
A place event's links are only shown to people who are in on it (going,
following its board, or standing nearby); everyone else sees that it has one.

### Meetings

Somewhere to gather, with links for whatever's going on: a call, a list, a song.
`help meetings` walks through the whole recipe, and the island nudges you along.
`make a meeting table` (or a gazebo) makes a meeting spot, and its maker, the
home's owner or a keeper leaves its links there:
`meet https://example.com/gathering https://example.com/snack-list` (up to
three; `meet off` clears them). Anyone standing there sees them when they look,
and `meet` hands them to you and tells everyone there you've headed off to the
meeting. Events can carry links too, `event here fri 4pm Tea party https://…`,
and one planned at a meeting table uses the table's. They show in `events`, the
reminder and the "starting now" shout, and `meet` at an event's spot while it's
on hands them over, each clickable on its own. In the browser, https links anywhere (lines and board
notes) open in a new tab.

### Gathering

Little finds pop up around anyone wandering, shown as pink `•` on the map:
flowers in meadows, butterflies on dry days, fireflies and moonflowers at night,
mushrooms in the woods after rain, shells on the shore, feathers on the hills,
crystals up on the rocks, plus seasonal finds and some that only grow on certain
islands. There are about 60 species, some rare.

- `gather` (or `pick` / `catch`) takes what's here. Butterflies sometimes flutter
  off to the next spot.
- `fish` next to water, then wait for a bite. Ponds and the sea have different fish.
- `satchel` holds 30 things. `journal` shows every species you've ever found.
- `give daisy to Poppy` (when you're together), `place daisy` to decorate,
  `release butterfly` to let it go.

### Cooking

Make a campfire (or a camp stove) or a clay oven wherever you fancy, then
`cook` to see what you've the makings of and `cook mint tea` to make it, from
things you've gathered as they are. It takes a few moments, and smells: as it
goes on, halfway, and when it's done, the smell drifts to everyone nearby.
Then there are a few helpings in your satchel to `eat`, or `serve` to everyone
standing with you. Wild mint, thyme and honeycomb turn up as you wander. Mint
tea, toasted acorns, grilled fish and mushroom stew cook on a fire; berry
crumble and honey cake in the oven.

### Companions

Wild animals turn up right beside people now and then: a fox in the woods, a
duckling on the water, a rabbit in the meadow by day, a hedgehog at night, a
turtle on the shore. Berries (blackberries, wild strawberries, bilberries) are
gathered like anything else, and `befriend fox` offers one; it may take a couple.
A companion follows you everywhere, gets up to little things, and shows up in
`look` and in your comings and goings. `petname Rusty`, `pet`, and `let go`.
One each, remembered between visits.

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
  `follow` / `unfollow` to change that, and `boards` lists the ones you follow
  (`unfollow 2` leaves one from anywhere).
- A board's followers are its group, like a group chat: anyone who follows it
  (or made it, or a keeper) can `add Poppy, Sam to board`, and they're told, or
  find a note waiting. They hear its notes and the place events planned there.
- **Hooks** let outside tools post too (builds, deploys, a calendar). At a
  board, its maker or a keeper types `hook CI` and is shown a secret address,
  once. Anything POSTed there lands on the board as a note from `🔔 CI`, like
  any other post: plain text, or JSON with `text` (Slack style) or `content`
  (Discord style), trimmed to 280 characters.
  `curl -d 'the build passed 🎉' https://world.example.com/hook/<secret>`
  `hooks` lists a board's hooks and when they last posted, and `unhook CI`
  stops one. It answers 200 when posted, 404 for a stopped or unknown hook,
  429 if one hook posts more than once every two seconds, and 413 past 16KB.
  Not on the Island Journal.

The **Island Journal** is a special board at the spawn point. The first find of
every species each week goes in it automatically, with a shout-out to the whole
island, and people can post underneath. It starts a fresh page every Monday:
`archive` lists past weeks and `read week 2026-10-26` opens one. When the journal is first put up it fills in every earlier week from
everyone's personal journals; `world.txt backfill-journal` does the same for a
journal that already exists (it's safe to run again).

## Island residents (AI)

Residents are AI characters who join like any other player and call Claude
(Haiku 5.5) to decide what to do. Each one has a character sheet in `personas/`.

```
export ANTHROPIC_API_KEY=...
world.txt invite Moss                    # once: Moss's account and invite
world.txt agent Moss --invite <link>     # the first time
world.txt agent Moss                     # after that (uses personas/moss.md)
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
world.txt bridge Claudia --invite <link>  # the first time (world.txt invite Claudia makes one)
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
  a magic link can talk to that session.
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
- `src/server/game/boards.rs`: noticeboards, their hooks and the Island Journal
- `src/server/gardens.rs` and `src/server/game/growing.rs`: plantings that grow over real days
- `src/server/game/meetings.rs`: meeting spots and their links
- `src/server/game/lighthouse.rs`: the lighthouse everyone mends together
- `src/agent.rs`: AI residents
- `src/bridge.rs`: `bridge` and `send`, for driving a resident from outside
- `.claude/skills/world-resident/`: the Claude Code skill
- `web/index.html`: the browser client

## License

MIT. See [LICENSE](LICENSE).
