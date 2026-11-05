# Hosting on Fly.io

A laptop world goes dark whenever the laptop sleeps. [Fly.io](https://fly.io)
keeps it up on one small always-on machine for a couple of dollars a month,
with the world on a volume. The repo has what it needs: a `Dockerfile`,
`deploy/fly-start.sh`, a settings template in `deploy/fly/example.toml` and
`deploy/fly-deploy` to deploy with.

Each world is its own Fly app, with its own settings file in `deploy/fly/`
(e.g. `deploy/fly/home.toml`). Those files are ignored by git, so your app
names and addresses stay on your machine.

## Setting up a world (once per world)

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

## Bringing an existing world across

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

## Deploying changes

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

## Keeper chores on Fly

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

---
[← back to the README](../README.md)
