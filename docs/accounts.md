# Accounts, invites and keepers

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

**What keepers can see.** Keepers can read `suggestions` and `reports`
(only what people chose to send them) and `discoveries` (who has tried which
feature, and when, with no other detail). Nobody can read anyone's
conversations. The full list is in [the rules](rules.md).

**Keeping it kind.** If someone's being unkind, `report Sam …` tells the
keepers what's been happening, in your own words: only the keepers see it
(`reports`), Sam isn't told, and nobody ever sees your conversations, only what
you wrote in the report. `mute Sam` means you stop hearing Sam's talk, emotes,
shouts, private messages, chat and board notes, or any music Sam plays on a
gramophone or jukebox. It's remembered, and Sam isn't told (`unmute Sam`,
`mutes`). Keepers can also `stop music` on anyone's gramophone, `skip` anyone's
jukebox song and switch off any radio. Keepers can `kick Sam` (30 minutes, or
`kick Sam 60`): Sam is sent off for a break and can't come back until it ends,
or a keeper types `unkick Sam`. Since only invited people get in, a kick or ban
can't be dodged with a new name.

---
[← back to the README](../README.md)
