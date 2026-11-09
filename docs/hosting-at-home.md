# Hosting on your own machine

`world.txt serve` runs a world on your own computer, at `http://localhost:4040`.
For friends outside your home to reach it, the world needs an address with https
(browsers insist on it for anything but localhost) that passes websockets
through. There are a few ways to get one:

- **A tunnel.** Your machine makes an outgoing connection to a tunnel service,
  which serves the world at a public address, so nothing on your router needs
  opening. [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)
  is free and is the one walked through below (`world.txt service install`
  knows how to run it). [Tailscale Funnel](https://tailscale.com/kb/1223/funnel),
  [ngrok](https://ngrok.com) and self-hosted ones like [frp](https://github.com/fatedier/frp)
  do the same job.
- **An open port.** If your home internet has a public address and you control
  the router, forward port 443 to a reverse proxy such as
  [Caddy](https://caddyserver.com), which fetches its own https certificate and
  passes websockets through untouched:

  ```
  world.example.com {
      reverse_proxy localhost:4040
  }
  ```

- **Family only.** With [Tailscale](https://tailscale.com) (or another private
  network), share the machine with them and they join at
  `http://<machine-name>:4040`, with nothing exposed to the internet at all.

Whichever you choose, tell the world its address (`world.txt serve --url
https://world.example.com`, or `WORLD_URL`), since it's in every magic link.
The rest of this page, from keeping it up to backups, applies whichever way
people get in.

The world is invite-only, so its address on its own doesn't let anyone in:
only the magic links you hand out do.

## Through a Cloudflare Tunnel

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
   do the same job, or see [Docker](#on-a-home-server-in-docker) below.)

## On a home server, in Docker

A machine that's always on (a home server, a NAS, a Raspberry Pi, or a rented
Linux box) can run the world in Docker, from the same image the
[Fly.io guide](hosting-fly.md) uses. Docker keeps the world and its tunnel
running side by side, brings them back if they fall over and starts them when
the machine boots. The world runs as an ordinary user inside the container, so
a bug in the server can only reach the world's own files.

On a Mac, `world.txt service install` (above) is the better fit: Docker there
runs inside a Linux virtual machine, which costs memory and still doesn't stop
the Mac from sleeping.

1. **Make the tunnel.** In Cloudflare's Zero Trust dashboard, create a tunnel
   of the `cloudflared` kind and copy the token it shows. Give the tunnel a
   public hostname, `world.example.com`, pointing at the service
   `http://island:8080` (the world's name inside Docker).

2. **Give the world a folder of its own,** next to wherever you cloned
   world.txt:

   ```
   mkdir ~/willow-bank && cd ~/willow-bank
   ```

   with this as `compose.yaml`:

   ```yaml
   services:
     island:
       build: ../world.txt          # wherever you cloned world.txt
       restart: unless-stopped
       environment:
         WORLD_URL: https://world.example.com
         TZ: Australia/Sydney       # the world's clock: festivals and Journal weeks follow it
       volumes:
         - ./data:/data             # the world, with its backups in data/backups

     tunnel:
       image: cloudflare/cloudflared:latest
       restart: unless-stopped
       command: tunnel --no-autoupdate run
       environment:
         TUNNEL_TOKEN: ${TUNNEL_TOKEN}
       depends_on: [island]
   ```

   and the token in `.env` beside it (keep that file to yourself: the token is
   all anyone needs to run your tunnel):

   ```
   TUNNEL_TOKEN=<the token from step 1>
   ```

3. **Start it:**

   ```
   docker compose up -d --build
   ```

   The first build compiles world.txt, which takes a few minutes (much longer
   on a Raspberry Pi). Then `curl https://world.example.com/.well-known/world.txt`
   should show your world.

4. **Make yourself a keeper,** running as the world's own user so everything
   it writes stays the world's:

   ```
   docker compose exec -u world island world.txt invite YourName --keeper
   ```

   Other keeper chores work the same way (`world.txt invites`, `world.txt
   rename 'Willow Bank'` and then `docker compose restart island`), and
   `docker compose logs -f island` shows what the world is saying.

- **Upgrading:** pull the latest world.txt, then `docker compose up -d --build`
  again. Everyone's browser waits a few seconds and comes straight back in.
- **Not using Cloudflare?** Leave out the `tunnel` service, give `island`
  `ports: ["4040:8080"]`, and point your reverse proxy or other tunnel at port 4040.
- **Backups** land in `data/backups/` as plain files on the machine itself, so
  copying them somewhere else is easy: see [keeping it up](#keeping-it-up).
  They belong to the container's user (uid 10001) but anyone can read them.
- **Bringing a world in:** stop the old server, copy its world into the new
  folder with `sqlite3 /path/to/old/world.db ".backup data/import.db"`, then
  `docker compose restart island`.
  The world moves it into place before it opens anything, and keeps the one it
  replaced in `data/replaced/`.

## Inviting people

- Make their account and link: `invite Sam` in the world (keepers), or
  `world.txt invite Sam --url https://world.example.com` on the server's machine.
- **Browser** (iPads too): send them the link. Opening it signs that device in.
- **Terminal:** `world.txt play --server wss://world.example.com --invite <link>`
  the first time (set `WORLD_SERVER` to skip `--server`), then just `world.txt play`.

## Keeping it up

- The world is only open while `world.txt serve` (and its tunnel, if it has one)
  is running, and while the machine is awake. On a Mac, `caffeinate -i world.txt serve` stops
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

---
[← back to the README](../README.md)
