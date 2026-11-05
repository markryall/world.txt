# Visiting other worlds

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

---
[← back to the README](../README.md)
