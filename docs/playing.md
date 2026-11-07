# Wandering the island

Everyone arrives on **The Village Green**, in the middle of Home Island. The
Island Journal stands there, with the village noticeboard and a long table for
gatherings beside it: the island's notes, hooks and plans can all live there,
and anyone can leave links on the table (`meet https://…`) for whatever happens
at it. Worlds that started before the green get one the next time they start,
wherever there's room around the Journal.

New arrivals get five tiny first steps: walk, say hi, make something, plant
something, see who's around. One hint shows at a time and clears when they do it.
In the browser the hint glows above the input, and tapping it does the step.
Progress is saved by name, so returning visitors never see it again
(`skip` hides it, `tutorial` brings it back).

- `n s e w ne nw se sw`, `look`, `who`, and `map` for the whole island at half
  scale: you, friends, homes, boards, and numbered named places
- `say hi` (or `'hi`), `:waves`, `shout`, `hum C E G`
- `@Poppy hi` talks to someone anywhere (in the browser, tapping someone's
  name in the log or the side panel starts one for you; if they're away it's kept as mail and
  shown when they next arrive), `reply` answers them, `chat` (or `@all`) reaches
  everyone, `find Poppy` tells you which way to walk, and `join Poppy` walks you
  there automatically (around the water, rowing across if they're on another
  island, following if they move; `stop` to stop).
  `places` lists every named spot and home on every island, and `go to The
  Greenhouse` (or `go 3`, or part of the name; "to" is optional) walks you
  there, down to the dock and rowing across first if it's on another island.
  In the browser, tap a name to message them.
- `ask How many ducks are on the pond?` drops a question into a hat for
  everyone standing with you. Each answers in secret (`answer 7`), and once
  everyone has, all the answers come out together, so nobody's swayed by
  anyone else's. Numbers come out smallest first. Anyone who arrives partway
  can join in by answering, someone who wanders off is out of it, and
  `reveal` shows what's in so far if someone's stepped away.
- `dig`, `fill`, `plant birch|bluebells|grass` (see Gardens), `raise`, `path`, `name The Willow Bank`
- `make a frog that croaks when it rains`, `push frog north`, `unmake frog`
- `make a music box`, then `teach C E G E C` (only its maker can) and `play`; it also
  plays now and then when people are near. Two people humming at the same spot
  are told they're making music together, and `hum along` keeps you in time
  (see [Humming along](#humming-along)). In the browser, tunes play as real notes.
- Tunes (for `hum`, `sing`, `whistle` and `teach`) are notes A to G, with `#`
  or `b` for sharp or flat and 2 to 6 for lower or higher (4 is the middle).
  A plain note is one beat. Each `-` holds it a beat longer (`G-` is two
  beats, `G---` four), each `/` halves it (`C/` half a beat, `C//` a quarter),
  and a `.` makes it half as long again (`E.`). `_` is a rest, and takes the
  same marks. `@100` anywhere sets the tempo, in beats a minute (40 to 240;
  120 if you don't say). So `hum @100 C C G G A A G- F F E E D D C-`. A music
  box remembers up to 48 notes.

A few old tunes to start with:

| Tune | Hum it |
|---|---|
| Twinkle, Twinkle, Little Star | `hum @110 C C G G A A G- F F E E D D C-` |
| Ode to Joy | `hum @120 E E F G G F E D C C D E E. D/ D- E E F G G F E D C C D E D. C/ C-` |
| Jingle Bells | `hum @160 E E E- E E E- E G C. D/ E--- F F F. F/ F E E E/ E/ E D D E D- G-` |
| Frère Jacques | `hum @120 C D E C C D E C E F G- E F G- G/ A/ G/ F/ E C G/ A/ G/ F/ E C C G3 C- C G3 C-` |
| Mary Had a Little Lamb | `hum @120 E D C D E E E- D D D- E G G- E D C D E E E E D D E D C---` |
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
  Inside a building it's one per room, heard only by whoever's in there, and it
  doesn't mind what's playing outside.
  Browser only, with the sound on; playing a gramophone needs a browser with
  WebCodecs (Chrome, Edge, recent Safari and Firefox).

Clouds drift across the island on the wind, and a whole day passes in 24 minutes.

In the browser, with the sound on (🔇 above the map), the island sounds like
where you are: rain, and thunder in a storm, the wind (strongest up on the
hills and down on the beach), waves by the sea, birds in the woods by day,
crickets in the grass at night, and the crackle of a campfire. Indoors you
hear the rain on the roof.

## Humming along

Two people typing a hum at the same moment never quite land together. In a
humming circle the world keeps the time: everyone's part plays at the start of
each turn, together, so it doesn't matter when you typed it.

- The songbook saves typing notes: `hum along canon bass`, `hum along canon
  tune`, `hum along frère jacques`, and Twinkle, Ode to Joy, Jingle Bells and
  Mary Had a Little Lamb. Plain `hum along` lists them, and in the browser
  you just tap one.
- `hum along @110 D3- A2- B2- F#2- G2- D2- G2- A2-` starts a circle where you
  stand. A turn lasts as long as that first tune (16 beats here) at its tempo,
  and that's how the circle goes round from then on.
- Everyone else `hum along`s a part of their own. It comes in at the next turn,
  at the circle's tempo (whatever tempo you typed).
- A part can have several **sections**, a turn each: `|` starts the next one,
  and a tune longer than a turn is cut into sections for you. Your sections go
  round and round in a loop until you `hush`, or walk off.
- In the browser, a strip above the box shows who's humming and which
  section they're on, with a line sweeping across each turn (tap 🎶 for the
  circle and your part, or a name to message them). The notes play without
  filling the log.
- `hum along` with nothing after it shows the circle and your sections,
  numbered. Hit a wrong note? `hum along 2 …` puts new notes in section 2, and
  it plays the new way next time round. In the browser, tap a section in the
  list and it's in the box ready to fix. `hum along 5 …` past the end adds one,
  `hush 2` takes section 2 out, and `hush` stops.
- A short section is fine: the rest of its turn is quiet. A note can't run
  over the end of a turn, though (you'll be told which section to fix), and
  `_` rests fill a turn where you'd like to sit one out.
- `sing along` and `whistle along` work too.

### Pachelbel's Canon

Pachelbel wrote it as one bass line going round and round (the *ground*)
while three violins play the same tune, each starting two bars after the last.
That's exactly a humming circle. The notes below are doubled in length from
Pachelbel's score so they sit nicely at `@110`, and each section is one time
round the ground: 16 beats, about 9 seconds.

It's in the songbook: one person taps (or types) `hum along canon bass`, the
others `hum along canon tune`, each a turn after the one before, and that's
the whole canon. Here are the notes, for humming it your own way.

The bass starts the circle with the ground, and hums it for ever:

```
hum along @110 D3- A2- B2- F#2- G2- D2- G2- A2-
```

The tune is ten sections, one after another, each two of Pachelbel's bars
(bars 3 to 22 of his violin part), and they get busier as they go:

| Section | | Notes |
|---|---|---|
| 1 | slow | `F#5- E5- D5- C#5- B4- A4- B4- C#5-` |
| 2 | slow | `D5- C#5- B4- A4- G4- F#4- G4- E4-` |
| 3 | two notes to each bass note | `D4 F#4 A4 G4 F#4 D4 F#4 E4 D4 B3 D4 A4 G4 B4 A4 G4` |
| 4 | two to each, leaping about | `F#4 D4 E4 C#5 D5 F#5 A5 A4 B4 G4 A4 F#4 D4 D5 D5. C#5/` |
| 5 | the quick run everyone knows, in eighth notes (`/`) | `D5/ C#5/ D5/ D4/ C#4/ A4/ E4/ F#4/ D4/ D5/ C#5/ B4/ C#5/ F#5/ A5/ B5/ G5/ F#5/ E5/ G5/ F#5/ E5/ D5/ C#5/ B4/ A4/ G4/ F#4/ E4/ G4/ F#4/ E4/` |
| 6 | more eighths, running up and down | `D4/ E4/ F#4/ G4/ A4/ E4/ A4/ G4/ F#4/ B4/ A4/ G4/ A4/ G4/ F#4/ E4/ D4/ B3/ B4/ C#5/ D5/ C#5/ B4/ A4/ G4/ F#4/ E4/ B4/ A4/ B4/ A4/ G4/` |
| 7 | slower again, climbing high | `F#4 F#5 E5- _ D5 F#5- B5- A5- B5- C#6-` |
| 8 | slower, with a rest to breathe | `D6 D5 C#5- _ B4 D5- D5-- D5 D5 G5 E5 A5` |
| 9 | **the very fast part**: flourishes in sixteenths (`//`) | `A5/ F#5// G5// A5/ F#5// G5// A5// A4// B4// C#5// D5// E5// F#5// G5// F#5/ D5// E5// F#5/ F#4// G4// A4// B4// A4// G4// A4// F#4// G4// A4// G4/ B4// A4// G4/ F#4// E4// F#4// E4// D4// E4// F#4// G4// A4// B4// G4/ B4// A4// B4/ C#5// D5// A4// B4// C#5// D5// E5// F#5// G5// A5//` |
| 10 | **the very fast part**, still going | `F#5/ D5// E5// F#5/ E5// D5// E5// C#5// D5// E5// F#5// E5// D5// C#5// D5/ B4// C#5// D5/ D4// E4// F#4// G4// F#4// E4// F#4// D5// C#5// D5// B4/ D5// C#5// B4/ A4// G4// A4// G4// F#4// G4// A4// B4// C#5// D5// B4/ D5// C#5// D5/ C#5// B4// C#5// D5// E5// D5// C#5// D5// B4// C#5//` |

That's a lot to type in one go, and you don't have to: each `hum along` adds
sections to the end of your part, so the first violin can start with

```
hum along F#5- E5- D5- C#5- B4- A4- B4- C#5- | D5- C#5- B4- A4- G4- F#4- G4- E4- | D4 F#4 A4 G4 F#4 D4 F#4 E4 D4 B3 D4 A4 G4 B4 A4 G4 | F#4 D4 E4 C#5 D5 F#5 A5 A4 B4 G4 A4 F#4 D4 D5 D5. C#5/
```

and then add `hum along <section 5> | <section 6>` and the rest while those
play. Keep ahead of yourself, though: after your last section your part goes
back round to the first.

The second violin types the very same sections once the first violin is
playing (any time during a turn is fine: they come in at the next one), and
the third the same after that. Each comes in a section behind the last, so
every violin plays over the one before it: the canon Pachelbel wrote. And
round it goes, for as long as you like.

Pachelbel goes on for many more bars than these, if you'd like to add them as
sections 11 and on. At this doubled-up scale his sixteenth notes are eighths
(`/`) and his thirty-second notes sixteenths (`//`), and a section can hold
up to 64 notes. (These notes are from Pachelbel's score as set by the Mutopia
Project from IMSLP, with every note held twice as long.)

### A round: Frère Jacques

In a round everyone hums the same tune and comes in a bit later. It's in the
songbook too (`hum along frère jacques`). Each
section of Frère Jacques is two bars (8 beats), so the first section sets the
turn:

```
hum along @120 C D E C C D E C | E F G- E F G- | G/ A/ G/ F/ E C G/ A/ G/ F/ E C | C G3 C- C G3 C-
```

Then a second person types the same thing, and a third, and a fourth, each
any time after the one before has come in. Four voices, each a section
apart, all going round.

### Fixing a mistake

Say Sam's section 2 has a sour F in it. `hum along` shows it:

```
🎶 The circle here goes round every 16 beats at @110.
Your part (section 3 is next):
   1  F#5- E5- D5- C#5- B4- A4- B4- C#5-
   2  D5- C#5- B4- A4- G4- F4- G4- E4-
   3  D4 F#4 A4 G4 F#4 D4 F#4 E4 D4 B3 D4 A4 G4 B4 A4 G4
```

and `hum along 2 D5- C#5- B4- A4- G4- F#4- G4- E4-` fixes it for next time
round, with nobody missing a beat.

## Homes

`claim` makes the 3x3 patch around you your home (one each, not too close to
anyone else's or to where newcomers arrive). It shows as `⌂` on the map and is
called "Poppy's Home" (with "Poppy's Garden" around it) unless you `name` it.
`home` (or `go home`) walks you back from anywhere, rowing across if it's on
another island. Anyone can visit, but only you,
the friends you `share` it with, and keepers can make, place, dig, plant or name
things there (`unshare`, `guests`). Claiming somewhere else moves your home.

Each spot of your home can have your own words, which anyone who looks there
sees in place of the land's: `describe A messy little den, records stacked
everywhere and fairy lights strung over the jukebox.` (up to 280 characters;
`describe` on its own puts it back).

Anything you've made can be painted, and so can your whole home: `paint the
shell cottage pink`, `paint the bench in sky blue`, `paint home lilac`. Say the
colour however you like (lilac, sea green, tangerine, rainbow…): your word is
what people read, drawn in the nearest of the island's colours, and a painted
home's `⌂` shows in its colour on the map. At your home you can paint what
your guests made too. `paint the bench plain` scrubs it off.

Homes can be furnished from finds, too: `recipes` lists things for a home
(a feather cushion, a toadstool stool, a leaf rug, a door wreath, a vase of
flowers, a shell mirror, a pinecone lamp, a driftwood table, a pebble doorstop,
a butterfly bush). `craft feather cushion`, then `place feather cushion` where
you want it, and paint it if you like.

Every finished building has a room inside: `go in` (or `enter the cottage`)
from beside it, and `out` (or any direction) to leave. Anyone can visit. A
room is a place like any other, just indoors: no weather, nothing that needs
open ground (no digging, planting, kites or building), and what's said inside
stays inside, except a `shout`, which carries just outside the door. Whoever may build at the home it's in (or its builder, if it's not
at a home) can `describe` the room in their own words and fill it with
furniture, cases and anything they make. Taking a building down sees anyone
inside safely out, and `undo` brings it back with its room as it was.

A home with a building in it looks lived in: its windows glow warm at night
while you're in the world, and smoke curls from the chimney while something's
cooking at your campfire or oven. If someone wanders into your home while you're
off elsewhere, you hear about it (`home` to wander back), at most once every
half hour for the same visitor.

Finds can be kept on show. `make a jar` (or a display case, a shelf, a
terrarium, a bug hotel, or a fish tank for fish), then `put firefly in jar` or
`put shell on shelf`. Anyone who looks there sees what's inside: butterflies
open and close their wings, beetles trundle about, fish turn circles, and
anything that glows (fireflies, glow-worms, glowcaps) lights up the dark. A case
holds twelve finds. `take firefly from jar` puts one back in your satchel. At
your home, you and your guests can fill each other's cases, but a find only
ever comes back out to whoever put it in, and unmaking a case hands everything
in it back to them.

## Gardens

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

## Islands and boats

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

## Seasons and festivals

The island's seasons turn every Monday (a whole year in a month): blossom in
spring, dragonflies in summer, falling leaves and acorns in autumn, frost and
snow in winter, each with its own finds, descriptions and weather. Every
Saturday is the season's festival (the Blossom Picnic, Midsummer Beach Night,
Lantern Night, Snow Lights), with festival-only keepsakes hidden around the
island that glow once placed. (`WORLD_SEASON=autumn` and `WORLD_FESTIVAL=1`
force them, for testing or a treat.)

Events (for the whole island, or just one spot) are in
[Events, gatherings and noticeboards](events-and-gatherings.md).

## The lighthouse

An old ruined lighthouse stands on Driftwood Isle's outermost point (Lighthouse
Point), and everyone mends it together, a stage at a time: a driftwood door,
stairs mended with smooth pebbles, a lens of quartz crystals, and then a light
kindled with something that glows (fire opals, glowcaps or glow-worms). Standing
there, `mend lighthouse` gives it what it needs next from your satchel. Looking at
it says what it's missing and roughly how far along it is, only ever in words,
and thanks everyone who has helped. Each finished stage is shouted to the whole
world and written in the Island Journal. Once it's lit, its beam sweeps across
the water at night, and it shows as `☼` on the map.

## Foraging

Little finds pop up around anyone wandering, shown as pink `•` on the map:
flowers in meadows, butterflies on dry days, fireflies and moonflowers at night,
mushrooms in the woods after rain, shells on the shore, feathers on the hills,
crystals up on the rocks, plus seasonal finds and some that only grow on certain
islands. There are about 60 species, some rare.

- `gather` (or `pick` / `catch`) takes what's here. Butterflies sometimes flutter
  off to the next spot.
- `fish` next to water, then wait for a bite. Ponds and the sea have different fish.
- `satchel` holds 30 things. `journal` shows every species you've ever found.
- `give daisy to Poppy` (when you're together; a resident sets your gift down
  there for everyone), `place daisy` to decorate,
  `release butterfly` to let it go.

## Cooking

Make a campfire (or a camp stove) or a clay oven wherever you fancy, then
`cook` to see what you've the makings of and `cook mint tea` to make it, from
things you've gathered as they are. It takes a few moments, and smells: as it
goes on, halfway, and when it's done, the smell drifts to everyone nearby.
Then there are a few helpings in your satchel to `eat`, or `serve` to everyone
standing with you. Wild mint, thyme and honeycomb turn up as you wander. Mint
tea, toasted acorns, grilled fish and mushroom stew cook on a fire; berry
crumble and honey cake in the oven.

## Companions

Wild animals turn up right beside people now and then: a fox in the woods, a
duckling on the water, a rabbit in the meadow by day, a hedgehog at night, a
turtle on the shore. Berries (blackberries, wild strawberries, bilberries) are
gathered like anything else, and `befriend fox` offers one; it may take a couple.
A companion follows you everywhere, gets up to little things, and shows up in
`look` and in your comings and goings. `petname Rusty`, `pet`, and `let go`.
One each, remembered between visits.

## Crafting

`recipes` shows what can be made from gathered things, ticking the ones your
satchel is ready for: flower crowns, bouquets, shell wind chimes, dreamcatchers,
crystal lanterns, mushroom soup, shell necklaces, stone cairns, a conch horn and
a rainbow garland. `craft wind chime` makes one (using your commonest finds
first, so rare ones aren't spent by accident). Crafts go in your satchel to
`give` or `place`, and placed crafts come alive: a wind chime tinkles in the
breeze, a lantern glows at night.

---
[← back to the README](../README.md)
