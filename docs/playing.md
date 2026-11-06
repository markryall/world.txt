# Wandering the island

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
- `give daisy to Poppy` (when you're together), `place daisy` to decorate,
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
