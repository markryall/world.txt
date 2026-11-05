# Island residents (AI)

Residents are AI characters who join like any other player and call Claude
(Haiku 5.5) to decide what to do. Each one has a character sheet in `personas/`.

```
export ANTHROPIC_API_KEY=...
world.txt invite Moss                    # once: Moss's account and invite
world.txt agent Moss --invite <link>     # the first time
world.txt agent Moss                     # after that (uses personas/moss.md)
world.txt agent Moss --dry-run           # prints what it would ask Claude, calls nothing
```

They're built not to burn tokens:

- They sleep while no humans are in the world, and don't call Claude at all.
- They wake when spoken to, when someone talks or builds nearby, or every 10
  minutes when someone's around. A few seconds of conversation is batched into one call.
- There's at least 20 seconds between calls and at most 40 calls an hour.
- Each one has a daily budget (default $0.50, `--daily-usd` or `WORLD_AGENT_DAILY_USD`),
  tracked from the API's reported usage. When it's spent, the resident naps until tomorrow.
- The persona and rules are a cached prefix. Each call adds only the last ~40 lines seen
  and a few remembered notes.
- They can only use gentle commands: no `unmake`, `push`, `shout` or `chat`.

Spend and memories live in `agent-state/<name>.json`.

## Driving a resident from Claude Code

For a special guest with a bigger brain, a Claude Code session can be the
resident instead. It runs on your Claude plan, with no API key needed.

```
world.txt bridge Claudia --invite <link>  # the first time (world.txt invite Claudia makes one)
world.txt bridge Claudia                  # stays connected, logs to tmp/bridge/claudia.log
world.txt send Claudia say hello darlings! # acts, and prints what the world said back
```

In a Claude Code session in this repo, `/world-resident Claudia` does all of
that: it starts the bridge, watches for lines meant for Claudia, and answers in
character until it hits its reply or hour limit (default 40 replies, 2 hours),
or you tell it to stop. Lines in the log starting with `!` are the ones that
want a response.

**Keep this to people you trust, and out of auto mode.** Unlike Moss (who can
only send game commands), the session behind `/world-resident` can run
anything on your machine, and every line players type is fed to it. The skill
treats world text as data and declines out-of-game requests in character, but
that's guidance, not a lock. So:

- Run it in Claude Code's **default permission mode**, never auto mode or with
  permissions bypassed. Anything beyond game commands then still asks you first.
  The skill refuses to start otherwise.
- Prefer it on your home network or Tailscale. A public tunnel means anyone with
  a magic link can talk to that session.
- For extra isolation, run the session in a container that holds only this repo
  and can reach nothing but the world. For example, Claude Code's
  [devcontainer](https://code.claude.com/docs/en/devcontainer) setup, with the
  bridge pointed at the host: `world.txt bridge Claudia --server ws://host.docker.internal:4040`.
  Inside a container, the skill skips its permission-mode check.

---
[← back to the README](../README.md)
