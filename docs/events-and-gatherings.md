# Events, gatherings and noticeboards

Everything for getting your people together: events (for the whole island or
just one spot), gathering tables with links, and noticeboards that work like
group chats. In the world, `help gatherings` has the short version.

## Events

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
the series. On a repeating event, `not going 1` skips just the next one
(`going 1` un-skips it), and `leave event 1` drops out for good.
`event here sun 10am Pancakes with Poppy, Sam and Lu` signs them up as going
too: they're told straight away, or find a note waiting when they next arrive.
Later, `add Poppy, Sam to event 1` signs more people up (its host or anyone
going can), and `remove Poppy from event 1` takes someone off (the host or a
keeper; anyone can drop themselves with `not going 1`).
A place event's links are only shown to people who are in on it (going,
following its board, or standing nearby); everyone else sees that it has one.

## Gatherings

Somewhere to gather, with links for whatever's going on: a call, a list, a song.
`help gatherings` walks through the whole recipe, and the island nudges you along.
`make a gathering table` (or a gazebo) makes a gathering spot, and its maker, the
home's owner or a keeper leaves its links there:
`meet https://example.com/gathering https://example.com/snack-list` (up to
three; `meet off` clears them). Anyone standing there sees them when they look,
and `meet` hands them to you and tells everyone there you've headed off to the
gathering. Events can carry links too, `event here fri 4pm Tea party https://…`,
and one planned at a gathering table uses the table's. They show in `events`, the
reminder and the "starting now" shout, and `meet` at an event's spot while it's
on hands them over, each clickable on its own. In the browser, https links anywhere (lines and board
notes) open in a new tab.

## Noticeboards

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

---
[← back to the README](../README.md)
