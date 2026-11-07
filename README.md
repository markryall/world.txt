# world.txt ✨

A small shared island made of text. You can wander, chat, dig ponds, plant trees,
name places and make things. There are no battles, just bonhomie.

```
brew install markryall/tap/world-txt  # or: make install (builds ~/.local/bin/world.txt)
world.txt serve                      # opens the world on :4040, saves to world.db
world.txt invite Mark --keeper       # your account, and its magic link: open it!
world.txt play --invite <link>       # or the terminal client (just `world.txt play` after)
```

Play in a browser (phones and iPads too) or in a terminal. The world is
invite-only: a keeper makes each account, and its magic link signs that device
in for good.

## What's it like?

Four little islands with weather, a day that passes in 24 minutes, seasons that
turn every week and a festival every Saturday. Wander about, gather flowers and
shells, plant a tree and watch it grow over real days, make a frog that croaks
when it rains, claim a home, befriend a fox, cook mint tea over a campfire, or
row across to another island. Talk to whoever's nearby, or to anyone, anywhere.

There are no points, coins or streaks, and nothing is lost for being away. Type
`help` for the basics; the rest is there to stumble on.

It's also a cosy place for a group to meet: plan events (one-offs or repeating,
in everyone's own timezone), leave links for calls at a gathering table, and keep
a noticeboard that works like a group chat, which outside tools can post to.

## Private by design

Nobody can read anyone else's conversations: not other players, and not the
island's keepers either. Private messages and mail go to the person they're
for and nowhere else. Keepers only ever see what someone chooses to send them
(a suggestion, say), plus a list of which features people have tried, with no
detail beyond that. [The rules](docs/rules.md) spell out exactly what's kept
and who can see it.

## More

- 🏝 [Wandering the island](docs/playing.md): homes, gardens, boats, foraging,
  cooking, companions, crafting, seasons and the lighthouse
- 🫖 [Events, gatherings and noticeboards](docs/events-and-gatherings.md):
  getting your people together
- 🗝 [Accounts, invites and keepers](docs/accounts.md): magic links,
  nicknames, and keeping it kind
- ⛵ [Visiting other worlds](docs/visiting.md): sail between islands run by
  different people
- 📏 [The rules](docs/rules.md): who can change what, every limit and every
  timing, for anyone who'd rather not be surprised
- 🔮 [Things to find](docs/secrets.md): the toys left out of `help` on purpose
  (spoilers!)
- ☁️ Hosting: on your own machine [through Cloudflare](docs/hosting-cloudflare.md),
  or always-on [on Fly.io](docs/hosting-fly.md)
- 🦊 [Island residents](docs/residents.md): AI characters who live there too
- 🛠 [Development](docs/development.md): how the code is laid out
- ⚙️ [How it works](docs/architecture.md): the game loop, the wire, the weather, and how tunes, rain, gramophones and radios make sound
- 📜 [Changelog](CHANGELOG.md)

## License

MIT. See [LICENSE](LICENSE).
