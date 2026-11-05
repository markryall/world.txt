# Hosting through Cloudflare

A [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)
lets people outside your network join without opening any ports on your router.
Your machine makes an outgoing connection to Cloudflare, and Cloudflare serves the
world over https and wss. Websockets work through it with no extra setup.

The world is invite-only, so the tunnel's address on its own doesn't let anyone
in: only the magic links you hand out do.

```
brew install cloudflared
```

## Quick tunnel (no account, link changes each time)

Good for an evening with friends.

```
world.txt serve
cloudflared tunnel --url http://localhost:4040
```

`cloudflared` prints a link like `https://some-random-words.trycloudflare.com`.
Stopping it ends the tunnel, and the next run gets a new link, which means
everyone's browser needs a fresh magic link too (browsers keep the device token
per address). For anything regular, use a named tunnel.

## Named tunnel (your own domain, same link every time)

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

## Your own domain, step by step

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

## Inviting people

- Make their account and link: `invite Sam` in the world (keepers), or
  `world.txt invite Sam --url https://world.example.com` on the server's machine.
- **Browser** (iPads too): send them the link. Opening it signs that device in.
- **Terminal:** `world.txt play --server wss://world.example.com --invite <link>`
  the first time (set `WORLD_SERVER` to skip `--server`), then just `world.txt play`.

## Keeping it up

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

---
[← back to the README](../README.md)
