# Changelog

## Unreleased

### Humming along
Hum together and actually stay in time. `hum along` starts a humming circle,
and everyone who joins adds a part. The world counts everyone in and plays all
the parts together at the start of every turn. A part can have sections
(`|` between them) that go round in a loop, so a ground bass, a canon or a
round all fit. Hit a wrong note? `hum along 2 …` fixes section 2 for next time
round, and `hush` stops. In the browser a little strip above the box shows
who's humming and which section they're on, with buttons to join and hush,
and the log stays free for talking. Pachelbel's Canon and Frère Jacques are in
[playing.md](docs/playing.md#humming-along) to try.

### A songbook
No need to type the notes: `hum along canon bass`, `hum along canon tune`,
`hum along frère jacques`, Twinkle, Ode to Joy, Jingle Bells and Mary Had a
Little Lamb. Plain `hum along` lists them, and in the browser you just tap
one, so joining Pachelbel's Canon from a phone is two taps. Tap one of your
own sections in that list and it's in the box, ready to fix a wrong note.

### The island on your home screen
Add to Home Screen on a phone gives the island its own icon (a little bit of
map, with you on it) and opens it full screen, with no browser bars.

### Tap the map to walk
Tap a spot on the map and you walk there, a step at a time, finding a way
around the water. No more tap, tap, tap on the arrows. (Underneath it's
`go +3 -2`, which works anywhere.)

### Tap a name to talk
In the browser, tap (or click) someone's name where they say, do, hum or
shout something, or arrive, and the box is ready with `@Sam ` to message
them.

### Fixes
- **Who am I?** The browser shows your name at the top of the side panel and
  in the tab (before the world's own name), so with a few windows open you
  can tell whose is whose.
- **The newest lines stay in view.** On a narrow screen the log could get
  left partway up when the panel above it filled in, so what came next landed
  out of sight. It stays at the bottom now, unless you've scrolled up to read.

## 0.7.0 (2026-11-07)

### Sounds of the island
Rain's not alone any more. With the sound on, you'll hear the wind gust
(strongest on the hills and the beach), waves roll in by the sea, thunder
rumble in a storm, birds in the woods by day, crickets in the grass at
night, and a campfire crackling. Indoors, the rain drums on the roof.

### Island folk
Someone new might row in to your island: a beachcomber, a painter, a
lamplighter, a fiddler… They potter about for a while, keep island hours,
make little things that don't last, and now and then throw a celebration.
Say hello.

### Humming with rhythm
Tunes can have long and short notes, rests and a tempo now, so real songs
fit: `-` holds a note longer, `/` makes it shorter, `.` dots it, `_` is a
rest and `@100` sets the tempo. `hum @100 C C G G A A G-`. Music boxes learn
them too, and remember twice as many notes. Plain tunes play a little slower
than before (120 beats a minute unless you say).

### Animals with lives of their own
Wild animals wander about now instead of waiting where they turn up, and get
on with their day when someone's watching. Some of them might even let you
pat them.

### How it works
A new [architecture doc](docs/architecture.md) tours the machinery: the
game loop, what travels over the wire, how the islands grow and the weather
drifts, and how tunes, rain, gramophones and radios make their sounds.

### Fixes
- **Quiet while you're away.** Companions used to keep getting up to their
  little tricks at people who'd stepped away. Now they, and the wandering
  animals, save it for when you're about.
- **Gifts to residents aren't lost.** Giving Claudia or Moss something used
  to make it vanish. Now they thank you and set it down right there for
  everyone to enjoy, or let it go if it's alive, or hand it back if there's
  no room.

## 0.6.0 (2026-11-07)

### Suggestions
`suggest …` leaves an idea, or something that muddled you, for the island's
keepers, who read them with `suggestions`. Only what you choose to send them
is ever seen.

### Reports
If someone's being unkind, `report Sam …` tells the keepers what's been
happening, in your own words. Keepers read them with `reports`. They see only
what you wrote, never your conversations.

### Discoveries
Keepers can see which features people have found with `discoveries`: just
that someone's tried each one, and when. Nothing about what they said, made or
where they were. It's a way to notice what's going unfound.

### Private by design
The README now says plainly what's always been true: nobody, keepers included,
can read anyone else's conversations.

### Music indoors
A gramophone, radio or jukebox inside a building plays just to the people in
that room. It can go right next to one outdoors, and outdoor music stays out.
One per room.

### Fixes
- **Rooms keep to themselves.** Rooms inside buildings sat so close together
  that a shout, a gramophone or the smell of cooking could carry from one to
  the next. They're far apart now (anything already in a room moves with it).
  A shout indoors fills the room and carries just outside the door; one from
  outside stays outside.
- **Home rules carry indoors.** Inside a building at your home, you can now do
  everything you can outdoors there: paint your guests' things, fill their
  display cases, and set the links on their gathering tables.
- **A typo can't cancel the wrong event.** `cancel Tea party`, `going x` and
  friends used to fall back to event 1 when they couldn't tell which you
  meant. Now an event is picked by its number, or by a name only one event has,
  and anything else asks which.
- **`paint home lilac` works from anywhere,** as it always meant to, even while
  you're visiting someone else's home.
- **Muting someone hides their humming** too, and any "making music together"
  that involves them.
- **Muting someone hides their noticeboard notes** when you `read` a board,
  not just as they're posted, old notes included.
- **The Village Green keeps its name.** Only keepers can rename the spot
  where everyone arrives.
- **Buildings count towards the twelve things a spot can hold,** like
  anything else you make or place.
- **A full satchel is full.** Being served, or taking a find out of a display
  case, now needs room, and finished cooking gives you what fits and leaves the
  rest by the fire for whoever comes by.
- **Pushing follows the rules where things land:** not into a full spot, not
  into a home that isn't yours to build in, and not out of a room into the
  nowhere around it.

## 0.5.0 (2026-11-07)

### Going inside
Every finished building has a room: `go in`, and `out` again. Describe it in
your own words, furnish it, paint things and fill your cases, rain or shine.
Friends can drop in, and what's said indoors stays indoors.

### The village green
Newcomers now arrive on The Village Green: the Island Journal in the middle,
with a village noticeboard and a long table for gatherings beside it, which
anyone can leave links on. A little village to arrive in, and a home for the
island's notes and plans, so the rest of the island is free for wandering.

### Little things for two
A few small things that work on your own and are nicer with someone else
there. The scenery will mention them when you've company.

### The hills
The high ground has a few things to find now. Climb up and see what the
scenery suggests.

### Paint
`paint the shell cottage pink`, `paint home lilac`: anything you've made, or
your whole home, in any colour you can name (rainbow included). People read
your word for it, things show in their colour wherever they're described, and
your home's `⌂` turns your colour on the map.

### Furniture from finds
`recipes` has a new section, for your home: feather cushions, toadstool
stools, leaf rugs, door wreaths, vases of flowers, shell mirrors that sparkle in
the sun, pinecone lamps that glow at night, driftwood tables, pebble doorstops,
and a butterfly bush for butterflies to drift in and out of.

### Homes that look lived in
A house glows at the windows at night while you're in the world, and its
chimney smokes while you've something cooking. If someone drops by your home
while you're off wandering, a little doorbell tells you who.

### Display cases
`make a jar`, then `put firefly in jar`: a home for your favourite finds,
where friends can see them. Butterflies open and close their wings, beetles
trundle about, a fish tank's fish turn slow circles, and a jar of fireflies
glows at night. Butterflies and bugs finally have somewhere to go. Guests at
your home can add to your cases, but a find only ever comes back out to
whoever put it in.

## 0.4.0 (2026-11-06)

### Things to stumble on
A handful of new bits of nonsense, left out of `help` on purpose: the scenery
drops the odd hint, and the rest is for finding. A beach, a windy day, a
puddle, a pond and a snowy week each have something going on. Anything that
pipes up by itself stays quiet for someone who's stepped away.

### A question in a hat
`ask Tea or coffee?` puts a question to everyone standing with you. Each
answers in secret (`answer tea`), and the answers all come out of the hat
together once everyone has. Numbers come out in order, and it says when
everyone agrees. There may be another game or two to find when you've company.

### Keeping things safe
- **Muting someone silences their music too:** `mute Sam` now stops you
  hearing Sam's gramophone and jukebox songs, as well as Sam's words.
- **A gentle pace:** a rush of twenty commands is fine, then four a second;
  past that the island asks you to slow down a little rather than kicking.
- **Visitors' passports** are only checked against home worlds out on the
  internet, never somewhere only the server can reach, and a visitor whose
  passport can't be checked is just told so.
- **Smaller messages:** connections that send far more than a browser ever
  would are let go, and the jukebox takes twenty songs from you at a time.
- **Residents** never pass on links, can't reach the whole world with
  `@everyone`, and keep what people tell them privately to themselves.
- **Under the hood:** the world runs as an ordinary user in its container
  rather than root, and finished connections are let go properly instead of
  lingering until the next restart.

### Links with words
`[the snack list](https://example.com/snacks)` shows as "the snack list" in the
browser, clickable, with the address on hover; the terminal shows the address
beside it. It works anywhere: talk, notes, events. No other markdown means
anything. Hooks turn Slack-style links (`<https://…|the cat cam>`) into the same
thing, and Slack's `<@someone|poppy>` into just the name.

### Building things
A satchel with enough in it can become somewhere to live: little houses
named for what they're made of, four or five finds at most, that go up over a
few real minutes while you watch the bar (or wander off). Some things only go
next door to a building, so a house can grow into a rambling sort of place.
Whoever built it can knock it down again. The scenery will say when you've
enough for something.

### Progress bars
`progress Pizza` at a noticeboard gives a secret address, like a hook, that
something outside posts `37/100` (or `37%`, or `3 of 10 photos`) to. Everyone
following the board, or standing near it, sees the bar fill in their side
panel, label on the right, without a word in their log. `clear Pizza` (or
posting `clear`) takes it away when it's done.

### Status lights
`status Surf` at a noticeboard gives an address that a surf report or a cat flap
posts `up` or `down` to, with a note if it likes (`down [the beach cam](https://…)
says flat`; Pingdom's alerts and Semaphore's webhook notifications work as
they are). Followers, and
anyone nearby, see a green or red light in their side panel and get a message
when it goes down or comes back up, never for staying up, and at most once a
minute however much it flaps. Two lights (or bars) with the same name on
different boards say where each one is. While something's down, there may be someone
on the noticeboard who'd appreciate a pat.

### Every new world its own shape
A brand-new world now grows from a random seed, so its coastlines, hills,
woods and ponds are its own (the islands keep their names). It's printed when
the world is first made; `--seed 7` still grows a twin of the old layout.
Existing worlds keep exactly the shape they have.

### Buttons for the things you do most
The buttons under the arrows are now 🧺 gather and 🎒 satchel (instead of
planting and dancing), so gathering is a click rather than a word.

### A shorter help
`help` is down to the essentials (walking, talking, gathering, making, home
and `mute`), plus the topics that need explaining: gatherings, noticeboards,
music and, for keepers, keeping. Everything else is there to be found: the
scenery now and then nudges you towards something you could do where you're
standing (a dig, a name, a boat to row, an animal that might like a berry),
never the same nudge twice in a visit. The other topics still answer if you
ask (`help cooking`), and `help all` still has everything.

## 0.3.0 (2026-11-05)

Reasons to come back, and a world that works across timezones. Still no
points, coins or streaks: just more to potter about with.

### Gatherings and events
- **Gathering tables:** `make a gathering table` (or a gazebo) and leave links there
  with `meet https://…`: a call, a list, a song, whatever the gathering needs
  (up to three). Anyone there sees them, and `meet` hands them over and tells
  everyone nearby you've headed off to the gathering.
- **Events can carry links** (`event fri 4pm Tea party https://…`), shown in
  `events`, the reminder and the "starting now" shout, each clickable. At a
  gathering table, the table's are used.
- **Place events:** `event here fri 4pm Tea party` is just for that spot. Only
  its host, people who've said `going 2`, followers of a noticeboard there and
  anyone nearby hear about it, so a little get-together doesn't ping the whole
  island. Anyone can plan one; whole-island events are still for keepers.
- **Repeating events:** `event here fridays 5pm Frog choir` (or `weekdays`,
  `weekends`, `daily`, `every mon wed`) stays at your 5pm and moves on to the
  next one by itself, keeping who's going and the links. `not going 1` skips
  just the next one, and `leave event 1` drops out for good.
- **Invites:** `event here sun 10am Pancakes with Poppy` signs Poppy up as
  going, with a note waiting if they're away. `add Sam to event 1` brings more
  people in later, and `remove Sam from event 1` takes them off. A place event's links only show
  to people who are in on it.
- **`help gatherings`** walks through making a spot for your group (a gathering
  table, a noticeboard beside it for the group, then events), and the island
  nudges you along as you go: making one suggests the other, and planning an
  event says who'll hear about it.
- **Everyone's own time:** browsers and the terminal client tell the world
  their timezone. 2pm means 2pm where you are, and everyone sees event times in
  theirs. A time that's already gone by says so. Your real date and time shows
  under the island's own clock, and `time Poppy` tells you what time it is for
  someone else, even while they're away.

### Getting about
- **`places`** lists every named spot and home on every island, and **`go to
  The Greenhouse`** (or `go 3`, or part of a name) walks you there, rowing
  across from the dock if it's on another island.

- **No more whisking:** `home` and `join Poppy` walk you there too, rowing
  across from the dock if it's on another island, so you see the way.

### Noticeboards
- **Hooks:** `hook Doorbell` at a board gives a secret address that outside tools
  (a doorbell, a smart kettle, a calendar) can POST notes to: plain text, or Slack- or
  Discord-style JSON. `hooks` lists them and `unhook` stops one.
- **Boards as groups:** anyone who follows a board can `add Poppy, Sam to
  board`, like adding someone to a group chat. `boards` numbers them, and
  `unfollow 2` leaves one from anywhere.

### Growing things
- **Gardens that grow:** trees go from sapling to young, tall and old over real
  days, and flowers from shoots to bloom over hours. Choose what you plant
  (`plant apple tree`, `plant bluebells`, and more). Nothing wilts, and you hear
  how your garden has grown, under "While you were away" if you've been gone.
- **The lighthouse:** an old ruin on Driftwood Isle that everyone mends
  together with `mend lighthouse`: a driftwood door, pebble stairs, a quartz
  lens, and a light. Progress is told in words, everyone who helped is
  thanked, and once it's lit its beam sweeps the water at night.

### The browser
- Everyone online is listed at the foot of the side panel with their local
  time; tap a name to talk to them. The buttons under the arrows are trimmed
  to help and the emoji ones.
- The side panel's top lines stay one line each, so a long place name can't
  push the map about.
- https links (a call, a build) are clickable and open in a new tab.

### Fixes
- Someone whose connection drops without a goodbye (a phone losing signal) now
  leaves the island, rather than lingering as a ghost until they come back.
- Hosted worlds keep their own clock (`TZ`), so festivals and the Journal's
  weeks follow your actual Saturdays and Mondays rather than UTC's.

## 0.2.1 (2026-11-03)

- **Restarts and dropped connections don't throw you out any more.** When the
  world restarts (say, for an update) it says so, and the browser waits and
  comes back in by itself, quietly, keeping everything on screen. A phone
  waking up or a wobbly connection does the same. The last 300 lines are also
  kept on your device, so even a refresh picks up where you were.
- A connection that falls far behind (a stalled phone, a rush of arrivals) can
  no longer run the whole world out of memory: its side panels are skipped
  until it catches up, and past a point it's let go to reconnect afresh.
- `describe` puts your own words on a spot at your home, which everyone who
  looks there sees (`describe` on its own puts the land's back).
- Much lighter on the server when lots of people are about: a step only
  refreshes the maps that can see it, at most twice a second, and the maps are
  half the size. In a load test on a small Fly.io machine, a world went from
  falling over at about 150 busy people to handling 300 comfortably.
- `dig` chips rock down to a hill, so no rock is stuck forever.
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
