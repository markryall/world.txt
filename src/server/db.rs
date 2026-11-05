//! Everything people make sticks around in SQLite.

use rusqlite::{Connection, params};

pub struct Db {
    conn: Connection,
}

pub struct TileRow {
    pub x: i64,
    pub y: i64,
    pub terrain: String,
    pub name: Option<String>,
}

pub struct ThingRow {
    pub id: i64,
    pub x: i64,
    pub y: i64,
    pub name: String,
    pub action: Option<String>,
    pub trigger: Option<String>,
    pub maker: String,
    /// "board" or "journal" for noticeboards; none for ordinary things.
    pub kind: Option<String>,
}

pub struct EventRow {
    pub id: i64,
    pub at: i64,
    pub name: String,
    pub x: i64,
    pub y: i64,
    pub host: String,
    pub reminded: bool,
    pub started: bool,
    /// A video call to join, if it has one.
    pub link: Option<String>,
    /// For one place and whoever's going, not the whole island.
    pub local: bool,
}

/// Something someone planted, growing over real days.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantingRow {
    pub x: i64,
    pub y: i64,
    /// What it is: "birch", "bluebells"...
    pub kind: String,
    pub by: String,
    pub at: i64,
    /// The last stage its planter was told about.
    pub told: i64,
}

pub struct NoteRow {
    pub id: i64,
    pub author: String,
    pub text: String,
    pub at: i64,
}

impl Db {
    pub fn open(path: &str) -> anyhow::Result<Db> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS tiles (
                 x INTEGER NOT NULL, y INTEGER NOT NULL,
                 terrain TEXT NOT NULL, name TEXT,
                 PRIMARY KEY (x, y));
             CREATE TABLE IF NOT EXISTS things (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 x INTEGER NOT NULL, y INTEGER NOT NULL,
                 name TEXT NOT NULL, action TEXT, trigger TEXT,
                 maker TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS players (
                 name TEXT PRIMARY KEY, x INTEGER NOT NULL, y INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS satchel (
                 player TEXT NOT NULL, species TEXT NOT NULL, count INTEGER NOT NULL,
                 PRIMARY KEY (player, species));
             CREATE TABLE IF NOT EXISTS notes (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 board INTEGER NOT NULL, author TEXT NOT NULL, text TEXT NOT NULL,
                 at INTEGER NOT NULL, week TEXT NOT NULL, species TEXT);
             CREATE INDEX IF NOT EXISTS notes_board ON notes (board, week, id);
             CREATE TABLE IF NOT EXISTS board_members (
                 player TEXT NOT NULL, board INTEGER NOT NULL,
                 following INTEGER NOT NULL DEFAULT 0, last_seen INTEGER NOT NULL DEFAULT 0,
                 PRIMARY KEY (player, board));
             DROP TABLE IF EXISTS knocks;
             DROP TABLE IF EXISTS door_knocks;
             CREATE TABLE IF NOT EXISTS mutes (
                 player TEXT NOT NULL, muted TEXT NOT NULL, PRIMARY KEY (player, muted));
             CREATE TABLE IF NOT EXISTS bans (
                 name TEXT NOT NULL, key_hash TEXT, until INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS homes (
                 owner TEXT PRIMARY KEY, x INTEGER NOT NULL, y INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS home_guests (
                 owner TEXT NOT NULL, guest TEXT NOT NULL, PRIMARY KEY (owner, guest));
             CREATE TABLE IF NOT EXISTS mail (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 recipient TEXT NOT NULL, sender TEXT NOT NULL, text TEXT NOT NULL, at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS companions (
                 owner TEXT PRIMARY KEY, animal TEXT NOT NULL, name TEXT);
             CREATE TABLE IF NOT EXISTS invites (
                 token_hash TEXT PRIMARY KEY, user TEXT NOT NULL, expires INTEGER NOT NULL,
                 used INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS devices (
                 token_hash TEXT PRIMARY KEY, user TEXT NOT NULL, created INTEGER NOT NULL, last_seen INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS events (
                 id INTEGER PRIMARY KEY AUTOINCREMENT, at INTEGER NOT NULL, name TEXT NOT NULL,
                 x INTEGER NOT NULL, y INTEGER NOT NULL, host TEXT NOT NULL,
                 reminded INTEGER NOT NULL DEFAULT 0, started INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE IF NOT EXISTS visitors (
                 name TEXT PRIMARY KEY, home TEXT NOT NULL, status TEXT NOT NULL, at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS trusted_worlds (
                 host TEXT PRIMARY KEY, by TEXT NOT NULL, at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS used_passports (
                 nonce TEXT PRIMARY KEY, expires INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS descriptions (
                 x INTEGER NOT NULL, y INTEGER NOT NULL, text TEXT NOT NULL,
                 by TEXT NOT NULL, at INTEGER NOT NULL,
                 PRIMARY KEY (x, y));
             CREATE TABLE IF NOT EXISTS stations (
                 name TEXT PRIMARY KEY COLLATE NOCASE, url TEXT NOT NULL, guide TEXT,
                 by TEXT NOT NULL, at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS journal (
                 player TEXT NOT NULL, species TEXT NOT NULL, found_at INTEGER NOT NULL,
                 PRIMARY KEY (player, species));
             CREATE TABLE IF NOT EXISTS hooks (
                 id INTEGER PRIMARY KEY AUTOINCREMENT, board INTEGER NOT NULL, name TEXT NOT NULL,
                 secret_hash TEXT NOT NULL UNIQUE, by TEXT NOT NULL, at INTEGER NOT NULL, last_at INTEGER);
             CREATE TABLE IF NOT EXISTS meeting_links (
                 thing INTEGER PRIMARY KEY, url TEXT NOT NULL, by TEXT NOT NULL, at INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS plantings (
                 x INTEGER NOT NULL, y INTEGER NOT NULL, kind TEXT NOT NULL, by TEXT NOT NULL,
                 at INTEGER NOT NULL, told INTEGER NOT NULL DEFAULT 0,
                 PRIMARY KEY (x, y));
             CREATE TABLE IF NOT EXISTS event_going (
                 event INTEGER NOT NULL, player TEXT NOT NULL, PRIMARY KEY (event, player));
             CREATE TABLE IF NOT EXISTS project_gifts (
                 project TEXT NOT NULL, stage INTEGER NOT NULL, player TEXT NOT NULL,
                 n INTEGER NOT NULL, at INTEGER NOT NULL);",
        )?;
        // Added later: first-steps progress. Players from before it existed
        // have clearly found their feet, so they start as done (99).
        let _ = conn.execute(
            "ALTER TABLE players ADD COLUMN tutorial INTEGER NOT NULL DEFAULT 99",
            [],
        );
        let _ = conn.execute("ALTER TABLE things ADD COLUMN kind TEXT", []);
        let _ = conn.execute("ALTER TABLE players ADD COLUMN key_hash TEXT", []);
        // Accounts: players.name is now a fixed username; the nickname is for show.
        let _ = conn.execute("ALTER TABLE players ADD COLUMN nickname TEXT", []);
        let _ = conn.execute("ALTER TABLE players ADD COLUMN keeper INTEGER NOT NULL DEFAULT 0", []);
        // Unmade things are tucked away rather than deleted, so they can come back.
        let _ = conn.execute("ALTER TABLE things ADD COLUMN removed_by TEXT", []);
        let _ = conn.execute("ALTER TABLE things ADD COLUMN removed_at INTEGER", []);
        // When an invite was opened, for keepers keeping track.
        let _ = conn.execute("ALTER TABLE invites ADD COLUMN used_at INTEGER", []);
        // A video call to go with an event.
        let _ = conn.execute("ALTER TABLE events ADD COLUMN link TEXT", []);
        // Events for one place (and whoever's going) rather than the whole island.
        let _ = conn.execute("ALTER TABLE events ADD COLUMN local INTEGER NOT NULL DEFAULT 0", []);
        // Where in the world someone last was, for times written to them while away.
        let _ = conn.execute("ALTER TABLE players ADD COLUMN tz TEXT", []);
        Ok(Db { conn })
    }

    /// Find someone by username or nickname (the username wins if both match).
    pub fn resolve_user(&self, name: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT name FROM players WHERE name = ?1 COLLATE NOCASE OR nickname = ?1 COLLATE NOCASE
                 ORDER BY (name = ?1 COLLATE NOCASE) DESC LIMIT 1",
                [name.trim()],
                |r| r.get(0),
            )
            .ok()
    }

    /// Everyone a name could mean, by username or nickname. More than one
    /// means the name is ambiguous (old accounts from before nicknames).
    pub fn matching_users(&self, name: &str) -> anyhow::Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT name FROM players WHERE name = ?1 COLLATE NOCASE OR nickname = ?1 COLLATE NOCASE
             ORDER BY (name = ?1 COLLATE NOCASE) DESC, name",
        )?;
        let rows = stmt.query_map([name.trim()], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Move everything `old` has into `new`, then remove `old`: satchel and
    /// journal merge, made things, notes, mail and devices move over, and
    /// `new` keeps their own home and companion if they already have one.
    pub fn fold_account(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO satchel (player, species, count) SELECT ?2, species, count FROM satchel WHERE player = ?1
             ON CONFLICT (player, species) DO UPDATE SET count = count + excluded.count",
            [old, new],
        )?;
        tx.execute(
            "INSERT INTO journal (player, species, found_at) SELECT ?2, species, found_at FROM journal WHERE player = ?1
             ON CONFLICT (player, species) DO UPDATE SET found_at = MIN(found_at, excluded.found_at)",
            [old, new],
        )?;
        for (table, col) in [
            ("things", "maker"),
            ("things", "removed_by"),
            ("notes", "author"),
            ("mail", "recipient"),
            ("mail", "sender"),
            ("events", "host"),
            ("devices", "user"),
            ("invites", "user"),
            ("trusted_worlds", "by"),
            ("bans", "name"),
            ("hooks", "by"),
            ("meeting_links", "by"),
            ("plantings", "by"),
            ("project_gifts", "player"),
        ] {
            tx.execute(&format!("UPDATE {table} SET {col} = ?2 WHERE {col} = ?1"), [old, new])?;
        }
        // Keyed tables: move what doesn't clash, drop what does.
        for (table, col) in [
            ("homes", "owner"),
            ("home_guests", "owner"),
            ("home_guests", "guest"),
            ("companions", "owner"),
            ("board_members", "player"),
            ("mutes", "player"),
            ("mutes", "muted"),
            ("event_going", "player"),
        ] {
            tx.execute(&format!("UPDATE OR IGNORE {table} SET {col} = ?2 WHERE {col} = ?1"), [old, new])?;
            tx.execute(&format!("DELETE FROM {table} WHERE {col} = ?1"), [old])?;
        }
        tx.execute("DELETE FROM satchel WHERE player = ?1", [old])?;
        tx.execute("DELETE FROM journal WHERE player = ?1", [old])?;
        tx.execute(
            "UPDATE players SET keeper = 1 WHERE name = ?2 AND EXISTS (SELECT 1 FROM players WHERE name = ?1 AND keeper = 1)",
            [old, new],
        )?;
        tx.execute("DELETE FROM players WHERE name = ?1", [old])?;
        tx.commit()?;
        Ok(())
    }

    // ---- radio stations ----------------------------------------------------

    /// Add a station to the world's list (or change its stream and guide).
    pub fn set_station(&self, name: &str, url: &str, guide: Option<&str>, by: &str, at: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO stations (name, url, guide, by, at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (name) DO UPDATE SET name = excluded.name, url = excluded.url, guide = excluded.guide, by = excluded.by, at = excluded.at",
            params![name, url, guide, by, at],
        )?;
        Ok(())
    }

    pub fn remove_station(&self, name: &str) -> anyhow::Result<bool> {
        Ok(self.conn.execute("DELETE FROM stations WHERE name = ?1", [name.trim()])? > 0)
    }

    /// The world's stations: (name, stream, guide), by name.
    pub fn stations(&self) -> anyhow::Result<Vec<(String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare("SELECT name, url, guide FROM stations ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ---- descriptions people write for places ---------------------------------

    pub fn set_description(&self, x: i64, y: i64, text: Option<&str>, by: &str, at: i64) -> anyhow::Result<()> {
        match text {
            Some(t) => self.conn.execute(
                "INSERT INTO descriptions (x, y, text, by, at) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(x, y) DO UPDATE SET text = excluded.text, by = excluded.by, at = excluded.at",
                params![x, y, t, by, at],
            )?,
            None => self.conn.execute("DELETE FROM descriptions WHERE x = ?1 AND y = ?2", params![x, y])?,
        };
        Ok(())
    }

    pub fn descriptions(&self) -> anyhow::Result<Vec<(i64, i64, String)>> {
        let mut stmt = self.conn.prepare("SELECT x, y, text FROM descriptions")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ---- invites and devices --------------------------------------------------

    pub fn add_invite(&self, user: &str, token_hash: &str, expires: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO invites (token_hash, user, expires) VALUES (?1, ?2, ?3)",
            params![token_hash, user, expires],
        )?;
        Ok(())
    }

    /// Use up an invite. The user it was for, if it's unused and in date.
    pub fn redeem_invite(&self, token_hash: &str, now: i64) -> Option<String> {
        let user: String = self
            .conn
            .query_row(
                "SELECT user FROM invites WHERE token_hash = ?1 AND used = 0 AND expires > ?2",
                params![token_hash, now],
                |r| r.get(0),
            )
            .ok()?;
        self.conn
            .execute("UPDATE invites SET used = 1, used_at = ?2 WHERE token_hash = ?1", params![token_hash, now])
            .ok()?;
        Some(user)
    }

    /// Recent invites, newest first: (username, nickname, expires, used_at).
    /// `used_at` is None while unused; old ones from before it was kept show 0.
    pub fn invites(&self, limit: usize) -> anyhow::Result<Vec<(String, String, i64, Option<i64>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT i.user, COALESCE(p.nickname, i.user), i.expires,
                    CASE WHEN i.used = 0 THEN NULL ELSE COALESCE(i.used_at, 0) END
             FROM invites i LEFT JOIN players p ON p.name = i.user
             ORDER BY i.expires DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Cancel someone's unused invites. How many there were.
    pub fn revoke_invites(&self, user: &str) -> anyhow::Result<usize> {
        Ok(self.conn.execute("DELETE FROM invites WHERE user = ?1 AND used = 0", [user])?)
    }

    pub fn add_device(&self, user: &str, token_hash: &str, now: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO devices (token_hash, user, created, last_seen) VALUES (?1, ?2, ?3, ?3)",
            params![token_hash, user, now],
        )?;
        Ok(())
    }

    /// Who a device token belongs to (and note that it was just used).
    pub fn device_user(&self, token_hash: &str, now: i64) -> Option<String> {
        let user: String = self
            .conn
            .query_row("SELECT user FROM devices WHERE token_hash = ?1", [token_hash], |r| r.get(0))
            .ok()?;
        let _ = self.conn.execute("UPDATE devices SET last_seen = ?2 WHERE token_hash = ?1", params![token_hash, now]);
        Some(user)
    }

    pub fn is_keeper(&self, user: &str) -> bool {
        self.conn
            .query_row("SELECT keeper FROM players WHERE name = ?1", [user], |r| r.get::<_, i64>(0))
            .is_ok_and(|k| k != 0)
    }

    pub fn set_keeper(&self, user: &str, keeper: bool) -> anyhow::Result<()> {
        self.conn.execute("UPDATE players SET keeper = ?2 WHERE name = ?1", params![user, keeper as i64])?;
        Ok(())
    }

    /// Sign someone out everywhere. How many devices were removed.
    pub fn remove_devices(&self, user: &str) -> anyhow::Result<usize> {
        Ok(self.conn.execute("DELETE FROM devices WHERE user = ?1", [user])?)
    }

    /// Everyone, for keepers: (username, nickname, keeper, devices, last seen).
    pub fn accounts(&self) -> anyhow::Result<Vec<(String, String, bool, i64, Option<i64>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.name, COALESCE(p.nickname, p.name), p.keeper,
                    (SELECT COUNT(*) FROM devices d WHERE d.user = p.name),
                    (SELECT MAX(last_seen) FROM devices d WHERE d.user = p.name)
             FROM players p ORDER BY p.keeper DESC, COALESCE(p.nickname, p.name) COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0, r.get(3)?, r.get(4)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn keeper_count(&self) -> i64 {
        self.conn.query_row("SELECT COUNT(*) FROM players WHERE keeper != 0", [], |r| r.get(0)).unwrap_or(0)
    }

    pub fn nickname(&self, user: &str) -> Option<String> {
        self.conn
            .query_row("SELECT COALESCE(nickname, name) FROM players WHERE name = ?1", [user], |r| r.get(0))
            .ok()
    }

    /// Everyone's (username, nickname).
    pub fn users(&self) -> anyhow::Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare("SELECT name, COALESCE(nickname, name) FROM players")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn create_user(&self, user: &str, nickname: &str, x: i64, y: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO players (name, nickname, x, y, tutorial) VALUES (?1, ?2, ?3, ?4, 0)",
            params![user, nickname, x, y],
        )?;
        Ok(())
    }

    pub fn set_nickname(&self, user: &str, nickname: &str) -> anyhow::Result<()> {
        self.conn.execute("UPDATE players SET nickname = ?2 WHERE name = ?1", params![user, nickname])?;
        Ok(())
    }

    pub fn meta(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .ok()
    }

    pub fn set_meta(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn tiles(&self) -> anyhow::Result<Vec<TileRow>> {
        let mut stmt = self.conn.prepare("SELECT x, y, terrain, name FROM tiles")?;
        let rows = stmt.query_map([], |r| {
            Ok(TileRow { x: r.get(0)?, y: r.get(1)?, terrain: r.get(2)?, name: r.get(3)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn save_tiles(&mut self, tiles: &[TileRow]) -> anyhow::Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO tiles (x, y, terrain, name) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(x, y) DO UPDATE SET terrain = excluded.terrain, name = excluded.name",
            )?;
            for t in tiles {
                stmt.execute(params![t.x, t.y, t.terrain, t.name])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn things(&self) -> anyhow::Result<Vec<ThingRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, x, y, name, action, trigger, maker, kind FROM things WHERE removed_at IS NULL")?;
        let rows = stmt.query_map([], |r| {
            Ok(ThingRow {
                id: r.get(0)?,
                x: r.get(1)?,
                y: r.get(2)?,
                name: r.get(3)?,
                action: r.get(4)?,
                trigger: r.get(5)?,
                maker: r.get(6)?,
                kind: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn add_thing(&self, t: &ThingRow) -> anyhow::Result<i64> {
        self.conn.execute(
            "INSERT INTO things (x, y, name, action, trigger, maker, kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![t.x, t.y, t.name, t.action, t.trigger, t.maker, t.kind],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn set_thing_action(&self, id: i64, action: Option<&str>) -> anyhow::Result<()> {
        self.conn.execute("UPDATE things SET action = ?2 WHERE id = ?1", params![id, action])?;
        Ok(())
    }

    pub fn move_thing(&self, id: i64, x: i64, y: i64) -> anyhow::Result<()> {
        self.conn
            .execute("UPDATE things SET x = ?2, y = ?3 WHERE id = ?1", params![id, x, y])?;
        Ok(())
    }

    pub fn remove_thing(&self, id: i64, by: &str) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64;
        self.conn.execute(
            "UPDATE things SET removed_by = ?2, removed_at = ?3 WHERE id = ?1",
            params![id, by, now],
        )?;
        Ok(())
    }

    /// The last thing someone unmade since a moment, for `undo`.
    pub fn last_removed(&self, by: &str, since: i64) -> Option<ThingRow> {
        self.conn
            .query_row(
                "SELECT id, x, y, name, action, trigger, maker, kind FROM things
                 WHERE removed_by = ?1 AND removed_at >= ?2 ORDER BY removed_at DESC, id DESC LIMIT 1",
                params![by, since],
                |r| {
                    Ok(ThingRow {
                        id: r.get(0)?,
                        x: r.get(1)?,
                        y: r.get(2)?,
                        name: r.get(3)?,
                        action: r.get(4)?,
                        trigger: r.get(5)?,
                        maker: r.get(6)?,
                        kind: r.get(7)?,
                    })
                },
            )
            .ok()
    }

    pub fn restore_thing(&self, id: i64) -> anyhow::Result<()> {
        self.conn
            .execute("UPDATE things SET removed_by = NULL, removed_at = NULL WHERE id = ?1", [id])?;
        Ok(())
    }

    /// What someone is carrying: (species id, count), in the order found.
    pub fn satchel(&self, player: &str) -> anyhow::Result<Vec<(String, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT species, count FROM satchel WHERE player = ?1 AND count > 0 ORDER BY rowid")?;
        let rows = stmt.query_map([player], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn satchel_add(&self, player: &str, species: &str, n: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO satchel (player, species, count) VALUES (?1, ?2, ?3)
             ON CONFLICT(player, species) DO UPDATE SET count = count + excluded.count",
            params![player, species, n],
        )?;
        self.conn.execute("DELETE FROM satchel WHERE count <= 0", [])?;
        Ok(())
    }

    // ---- noticeboards ---------------------------------------------------

    pub fn add_note(
        &self,
        board: i64,
        author: &str,
        text: &str,
        at: i64,
        week: &str,
        species: Option<&str>,
    ) -> anyhow::Result<i64> {
        self.conn.execute(
            "INSERT INTO notes (board, author, text, at, week, species) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![board, author, text, at, week, species],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Up to `limit` notes before `before` (newest first in the query, returned
    /// oldest first), optionally one week's page only. Also says whether older
    /// ones exist.
    pub fn notes(
        &self,
        board: i64,
        week: Option<&str>,
        before: Option<i64>,
        limit: i64,
    ) -> anyhow::Result<(Vec<NoteRow>, bool)> {
        let mut stmt = self.conn.prepare(
            "SELECT id, author, text, at FROM notes
             WHERE board = ?1 AND (?2 IS NULL OR week = ?2) AND id < ?3
             ORDER BY id DESC LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![board, week, before.unwrap_or(i64::MAX), limit + 1], |r| {
            Ok(NoteRow { id: r.get(0)?, author: r.get(1)?, text: r.get(2)?, at: r.get(3)? })
        })?;
        let mut notes: Vec<NoteRow> = rows.collect::<Result<_, _>>()?;
        let more = notes.len() as i64 > limit;
        notes.truncate(limit as usize);
        notes.reverse();
        Ok((notes, more))
    }

    pub fn notes_after(&self, board: i64, after: i64) -> anyhow::Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE board = ?1 AND id > ?2",
            params![board, after],
            |r| r.get(0),
        )?)
    }

    pub fn last_note(&self, board: i64) -> anyhow::Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM notes WHERE board = ?1",
            [board],
            |r| r.get(0),
        )?)
    }

    /// Past weeks with notes, newest first: (week, count).
    pub fn weeks(&self, board: i64) -> anyhow::Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT week, COUNT(*) FROM notes WHERE board = ?1 GROUP BY week ORDER BY week DESC",
        )?;
        let rows = stmt.query_map([board], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn species_noted(&self, board: i64, species: &str, week: &str) -> anyhow::Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM notes WHERE board = ?1 AND species = ?2 AND week = ?3)",
            params![board, species, week],
            |r| r.get(0),
        )?)
    }

    /// (following, last seen note id), if this player has been at this board.
    pub fn membership(&self, player: &str, board: i64) -> Option<(bool, i64)> {
        self.conn
            .query_row(
                "SELECT following, last_seen FROM board_members WHERE player = ?1 AND board = ?2",
                params![player, board],
                |r| Ok((r.get::<_, i64>(0)? != 0, r.get(1)?)),
            )
            .ok()
    }

    pub fn set_seen(&self, player: &str, board: i64, last_seen: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO board_members (player, board, last_seen) VALUES (?1, ?2, ?3)
             ON CONFLICT(player, board) DO UPDATE SET last_seen = MAX(last_seen, excluded.last_seen)",
            params![player, board, last_seen],
        )?;
        Ok(())
    }

    pub fn set_following(&self, player: &str, board: i64, following: bool) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO board_members (player, board, following) VALUES (?1, ?2, ?3)
             ON CONFLICT(player, board) DO UPDATE SET following = excluded.following",
            params![player, board, following as i64],
        )?;
        Ok(())
    }

    pub fn followers(&self, board: i64) -> anyhow::Result<Vec<String>> {
        let mut stmt =
            self.conn.prepare("SELECT player FROM board_members WHERE board = ?1 AND following = 1")?;
        let rows = stmt.query_map([board], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Boards someone follows: (board, last seen note id).
    pub fn followed(&self, player: &str) -> anyhow::Result<Vec<(i64, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT board, last_seen FROM board_members WHERE player = ?1 AND following = 1")?;
        let rows = stmt.query_map([player], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Everyone's discoveries since a moment: (player, species, found_at), oldest first.
    pub fn discoveries_since(&self, since: i64) -> anyhow::Result<Vec<(String, String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT player, species, found_at FROM journal WHERE found_at >= ?1 ORDER BY found_at",
        )?;
        let rows = stmt.query_map([since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Note a discovery. True if it's the first of its kind for this player.
    pub fn journal_add(&self, player: &str, species: &str) -> anyhow::Result<bool> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64;
        let n = self.conn.execute(
            "INSERT OR IGNORE INTO journal (player, species, found_at) VALUES (?1, ?2, ?3)",
            params![player, species, now],
        )?;
        Ok(n > 0)
    }

    pub fn journal(&self, player: &str) -> anyhow::Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT species FROM journal WHERE player = ?1")?;
        let rows = stmt.query_map([player], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Where a returning player was, and how far through first steps.
    // ---- companions ---------------------------------------------------------

    pub fn companion(&self, owner: &str) -> Option<(String, Option<String>)> {
        self.conn
            .query_row("SELECT animal, name FROM companions WHERE owner = ?1", [owner], |r| Ok((r.get(0)?, r.get(1)?)))
            .ok()
    }

    pub fn set_companion(&self, owner: &str, animal: Option<&str>, name: Option<&str>) -> anyhow::Result<()> {
        match animal {
            Some(a) => self.conn.execute(
                "INSERT INTO companions (owner, animal, name) VALUES (?1, ?2, ?3)
                 ON CONFLICT(owner) DO UPDATE SET animal = excluded.animal, name = excluded.name",
                params![owner, a, name],
            )?,
            None => self.conn.execute("DELETE FROM companions WHERE owner = ?1", [owner])?,
        };
        Ok(())
    }

    // ---- visitors from other worlds ---------------------------------------------

    /// Note a passport as used. False if it already was.
    pub fn use_passport(&self, nonce: &str, expires: i64, now: i64) -> anyhow::Result<bool> {
        self.conn.execute("DELETE FROM used_passports WHERE expires < ?1", [now])?;
        Ok(self.conn.execute("INSERT OR IGNORE INTO used_passports (nonce, expires) VALUES (?1, ?2)", params![nonce, expires])? > 0)
    }

    /// "waiting", "admitted" or "turned away", if they've been here before.
    pub fn visitor_status(&self, name: &str) -> Option<String> {
        self.conn.query_row("SELECT status FROM visitors WHERE name = ?1", [name], |r| r.get(0)).ok()
    }

    pub fn set_visitor(&self, name: &str, home: &str, status: &str, now: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO visitors (name, home, status, at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET status = excluded.status, at = excluded.at",
            params![name, home, status, now],
        )?;
        Ok(())
    }

    pub fn visitor_home(&self, name: &str) -> Option<String> {
        self.conn.query_row("SELECT home FROM visitors WHERE name = ?1", [name], |r| r.get(0)).ok()
    }

    /// Visitors, most recent first: (name, home, status).
    pub fn visitors(&self) -> anyhow::Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare("SELECT name, home, status FROM visitors ORDER BY at DESC LIMIT 30")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Is this world (by its host, e.g. "ryall.island") trusted to come straight in?
    pub fn is_trusted(&self, home: &str) -> bool {
        let host = super::federation::host(home);
        self.conn
            .query_row("SELECT EXISTS (SELECT 1 FROM trusted_worlds WHERE host = ?1)", [host], |r| r.get(0))
            .unwrap_or(false)
    }

    pub fn set_trusted(&self, home: &str, by: &str, trusted: bool, now: i64) -> anyhow::Result<bool> {
        let host = super::federation::host(home);
        let n = if trusted {
            self.conn.execute(
                "INSERT OR IGNORE INTO trusted_worlds (host, by, at) VALUES (?1, ?2, ?3)",
                params![host, by, now],
            )?
        } else {
            self.conn.execute("DELETE FROM trusted_worlds WHERE host = ?1", [host])?
        };
        Ok(n > 0)
    }

    /// Trusted worlds: (host, who trusted it).
    pub fn trusted_worlds(&self) -> anyhow::Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare("SELECT host, by FROM trusted_worlds ORDER BY host")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ---- events ----------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub fn add_event(&self, at: i64, name: &str, x: i64, y: i64, host: &str, link: Option<&str>, local: bool) -> anyhow::Result<i64> {
        self.conn.execute(
            "INSERT INTO events (at, name, x, y, host, link, local) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![at, name, x, y, host, link, local],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Who's said they're going to an event, in the order they said it.
    pub fn going(&self, event: i64) -> anyhow::Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT player FROM event_going WHERE event = ?1 ORDER BY rowid")?;
        let rows = stmt.query_map([event], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Say you're going (or not). True if that changed anything.
    pub fn set_going(&self, event: i64, player: &str, going: bool) -> anyhow::Result<bool> {
        let n = if going {
            self.conn.execute("INSERT OR IGNORE INTO event_going (event, player) VALUES (?1, ?2)", params![event, player])?
        } else {
            self.conn.execute("DELETE FROM event_going WHERE event = ?1 AND player = ?2", params![event, player])?
        };
        Ok(n > 0)
    }

    /// Events from `since` on, soonest first.
    pub fn upcoming_events(&self, since: i64) -> anyhow::Result<Vec<EventRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, at, name, x, y, host, reminded, started, link, local FROM events WHERE at >= ?1 ORDER BY at",
        )?;
        let rows = stmt.query_map([since], |r| {
            Ok(EventRow {
                id: r.get(0)?,
                at: r.get(1)?,
                name: r.get(2)?,
                x: r.get(3)?,
                y: r.get(4)?,
                host: r.get(5)?,
                reminded: r.get::<_, i64>(6)? != 0,
                started: r.get::<_, i64>(7)? != 0,
                link: r.get(8)?,
                local: r.get::<_, i64>(9)? != 0,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn mark_event(&self, id: i64, reminded: bool, started: bool) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE events SET reminded = MAX(reminded, ?2), started = MAX(started, ?3) WHERE id = ?1",
            params![id, reminded as i64, started as i64],
        )?;
        Ok(())
    }

    pub fn remove_event(&self, id: i64) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM events WHERE id = ?1", [id])?;
        self.conn.execute("DELETE FROM event_going WHERE event = ?1", [id])?;
        Ok(())
    }

    pub fn forget_events_before(&self, before: i64) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM events WHERE at < ?1", [before])?;
        self.conn.execute("DELETE FROM event_going WHERE event NOT IN (SELECT id FROM events)", [])?;
        Ok(())
    }

    // ---- mail ----------------------------------------------------------------

    pub fn send_mail(&self, recipient: &str, sender: &str, text: &str, at: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO mail (recipient, sender, text, at) VALUES (?1, ?2, ?3, ?4)",
            params![recipient, sender, text, at],
        )?;
        // Keep the newest 50 for anyone, so a mailbox can't be flooded.
        self.conn.execute(
            "DELETE FROM mail WHERE recipient = ?1 AND id NOT IN
             (SELECT id FROM mail WHERE recipient = ?1 ORDER BY id DESC LIMIT 50)",
            [recipient],
        )?;
        Ok(())
    }

    /// Everything waiting for someone, oldest first, and it's then delivered.
    pub fn take_mail(&self, recipient: &str) -> anyhow::Result<Vec<(String, String, i64)>> {
        let mut stmt = self.conn.prepare("SELECT sender, text, at FROM mail WHERE recipient = ?1 ORDER BY id")?;
        let rows: Vec<(String, String, i64)> =
            stmt.query_map([recipient], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
        self.conn.execute("DELETE FROM mail WHERE recipient = ?1", [recipient])?;
        Ok(rows)
    }

    // ---- homes ---------------------------------------------------------------

    /// Every home: (owner, x, y, guests).
    pub fn homes(&self) -> anyhow::Result<Vec<(String, i64, i64, Vec<String>)>> {
        let mut stmt = self.conn.prepare("SELECT owner, x, y FROM homes")?;
        let rows: Vec<(String, i64, i64)> =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
        let mut out = Vec::new();
        for (owner, x, y) in rows {
            let mut g = self.conn.prepare("SELECT guest FROM home_guests WHERE owner = ?1 ORDER BY guest")?;
            let guests = g.query_map([&owner], |r| r.get(0))?.collect::<Result<_, _>>()?;
            out.push((owner, x, y, guests));
        }
        Ok(out)
    }

    pub fn set_home(&self, owner: &str, x: i64, y: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO homes (owner, x, y) VALUES (?1, ?2, ?3)
             ON CONFLICT(owner) DO UPDATE SET x = excluded.x, y = excluded.y",
            params![owner, x, y],
        )?;
        Ok(())
    }

    pub fn set_guest(&self, owner: &str, guest: &str, on: bool) -> anyhow::Result<()> {
        if on {
            self.conn.execute("INSERT OR IGNORE INTO home_guests (owner, guest) VALUES (?1, ?2)", params![owner, guest])?;
        } else {
            self.conn.execute("DELETE FROM home_guests WHERE owner = ?1 AND guest = ?2 COLLATE NOCASE", params![owner, guest])?;
        }
        Ok(())
    }

    // ---- mutes and kicks ----------------------------------------------------

    pub fn mutes(&self, player: &str) -> anyhow::Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT muted FROM mutes WHERE player = ?1 ORDER BY muted")?;
        let rows = stmt.query_map([player], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn set_mute(&self, player: &str, muted: &str, on: bool) -> anyhow::Result<()> {
        if on {
            self.conn.execute("INSERT OR IGNORE INTO mutes (player, muted) VALUES (?1, ?2)", params![player, muted])?;
        } else {
            self.conn.execute(
                "DELETE FROM mutes WHERE player = ?1 AND muted = ?2 COLLATE NOCASE",
                params![player, muted],
            )?;
        }
        Ok(())
    }

    pub fn add_ban(&self, name: &str, key_hash: Option<&str>, until: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO bans (name, key_hash, until) VALUES (?1, ?2, ?3)",
            params![name, key_hash, until],
        )?;
        Ok(())
    }

    /// When a ban on this name or device ends, if one is still going.
    pub fn banned_until(&self, name: &str, key_hash: Option<&str>, now: i64) -> Option<i64> {
        self.conn
            .query_row(
                "SELECT MAX(until) FROM bans
                 WHERE until > ?3 AND (name = ?1 COLLATE NOCASE OR (?2 IS NOT NULL AND key_hash = ?2))",
                params![name, key_hash, now],
                |r| r.get::<_, Option<i64>>(0),
            )
            .ok()
            .flatten()
    }

    /// Lift any ban on a name (and the devices banned along with it).
    pub fn lift_ban(&self, name: &str) -> anyhow::Result<bool> {
        let n = self.conn.execute("DELETE FROM bans WHERE name = ?1 COLLATE NOCASE", [name])?;
        Ok(n > 0)
    }

    pub fn player(&self, name: &str) -> Option<(i64, i64, u8)> {
        self.conn
            .query_row("SELECT x, y, tutorial FROM players WHERE name = ?1", [name], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .ok()
    }

    pub fn zone(&self, name: &str) -> Option<String> {
        self.conn.query_row("SELECT tz FROM players WHERE name = ?1", [name], |r| r.get(0)).ok().flatten()
    }

    pub fn set_zone(&self, name: &str, tz: &str) -> anyhow::Result<()> {
        self.conn.execute("UPDATE players SET tz = ?2 WHERE name = ?1", params![name, tz])?;
        Ok(())
    }

    pub fn save_player(&self, name: &str, x: i64, y: i64, tutorial: u8) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO players (name, x, y, tutorial) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET x = excluded.x, y = excluded.y, tutorial = excluded.tutorial",
            params![name, x, y, tutorial],
        )?;
        Ok(())
    }

    // ---- hooks: outside tools posting to boards ------------------------------

    pub fn add_hook(&self, board: i64, name: &str, secret_hash: &str, by: &str, at: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO hooks (board, name, secret_hash, by, at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![board, name, secret_hash, by, at],
        )?;
        Ok(())
    }

    /// A board's hooks: (name, made by, made at, last posted).
    pub fn hooks(&self, board: i64) -> anyhow::Result<Vec<(String, String, i64, Option<i64>)>> {
        let mut stmt = self.conn.prepare("SELECT name, by, at, last_at FROM hooks WHERE board = ?1 ORDER BY id")?;
        let rows = stmt.query_map([board], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn remove_hook(&self, board: i64, name: &str) -> anyhow::Result<bool> {
        Ok(self.conn.execute("DELETE FROM hooks WHERE board = ?1 AND name = ?2 COLLATE NOCASE", params![board, name])? > 0)
    }

    /// The hook a secret belongs to: (id, board, name).
    pub fn hook_by_secret(&self, secret_hash: &str) -> Option<(i64, i64, String)> {
        self.conn
            .query_row("SELECT id, board, name FROM hooks WHERE secret_hash = ?1", [secret_hash], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .ok()
    }

    pub fn touch_hook(&self, id: i64, at: i64) -> anyhow::Result<()> {
        self.conn.execute("UPDATE hooks SET last_at = ?2 WHERE id = ?1", params![id, at])?;
        Ok(())
    }

    // ---- meeting spots -------------------------------------------------------

    pub fn meeting_links(&self) -> anyhow::Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare("SELECT thing, url FROM meeting_links")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn set_meeting_link(&self, thing: i64, url: Option<&str>, by: &str, at: i64) -> anyhow::Result<()> {
        match url {
            Some(u) => self.conn.execute(
                "INSERT INTO meeting_links (thing, url, by, at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(thing) DO UPDATE SET url = excluded.url, by = excluded.by, at = excluded.at",
                params![thing, u, by, at],
            )?,
            None => self.conn.execute("DELETE FROM meeting_links WHERE thing = ?1", [thing])?,
        };
        Ok(())
    }

    // ---- gardens -------------------------------------------------------------

    pub fn plantings(&self) -> anyhow::Result<Vec<PlantingRow>> {
        let mut stmt = self.conn.prepare("SELECT x, y, kind, by, at, told FROM plantings")?;
        let rows = stmt.query_map([], |r| {
            Ok(PlantingRow { x: r.get(0)?, y: r.get(1)?, kind: r.get(2)?, by: r.get(3)?, at: r.get(4)?, told: r.get(5)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn set_planting(&self, p: &PlantingRow) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO plantings (x, y, kind, by, at, told) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(x, y) DO UPDATE SET kind = excluded.kind, by = excluded.by, at = excluded.at, told = excluded.told",
            params![p.x, p.y, p.kind, p.by, p.at, p.told],
        )?;
        Ok(())
    }

    pub fn remove_planting(&self, x: i64, y: i64) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM plantings WHERE x = ?1 AND y = ?2", params![x, y])?;
        Ok(())
    }

    // ---- shared projects (the lighthouse) ------------------------------------

    pub fn add_project_gift(&self, project: &str, stage: i64, player: &str, n: i64, at: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO project_gifts (project, stage, player, n, at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![project, stage, player, n, at],
        )?;
        Ok(())
    }

    /// Everything given towards one stage: (player, how many), first giver first.
    pub fn project_gifts(&self, project: &str, stage: i64) -> anyhow::Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT player, SUM(n) FROM project_gifts WHERE project = ?1 AND stage = ?2 GROUP BY player ORDER BY MIN(rowid)",
        )?;
        let rows = stmt.query_map(params![project, stage], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_an_old_account_moves_its_things_and_removes_it() {
        let db = Db::open(":memory:").unwrap();
        // An old account (username is a plain name) and a new one nicknamed the same.
        db.conn.execute("INSERT INTO players (name, x, y, keeper) VALUES ('Poppy', 1, 1, 1)", []).unwrap();
        db.create_user("sunny-otter-42", "Poppy", 2, 2).unwrap();
        assert_eq!(db.matching_users("poppy").unwrap(), vec!["Poppy", "sunny-otter-42"]);

        db.satchel_add("Poppy", "daisy", 2).unwrap();
        db.satchel_add("sunny-otter-42", "daisy", 1).unwrap();
        db.journal_add("Poppy", "frog").unwrap();
        db.send_mail("Poppy", "Sam", "hi", 5).unwrap();

        db.fold_account("Poppy", "sunny-otter-42").unwrap();

        assert_eq!(db.matching_users("Poppy").unwrap(), vec!["sunny-otter-42"]);
        assert_eq!(db.satchel("sunny-otter-42").unwrap(), vec![("daisy".to_string(), 3)]);
        assert_eq!(db.journal("sunny-otter-42").unwrap(), vec!["frog"]);
        assert_eq!(db.take_mail("sunny-otter-42").unwrap().len(), 1);
        assert!(db.is_keeper("sunny-otter-42"), "keeping keeper powers from the old account");
        assert!(db.satchel("Poppy").unwrap().is_empty());
    }

    #[test]
    fn whos_going_is_kept_with_the_event_and_goes_with_it() {
        let db = Db::open(":memory:").unwrap();
        let e = db.add_event(100, "Standup", 1, 2, "host", None, true).unwrap();
        assert!(db.set_going(e, "poppy", true).unwrap());
        assert!(!db.set_going(e, "poppy", true).unwrap(), "already going");
        assert!(db.set_going(e, "sam", true).unwrap());
        assert_eq!(db.going(e).unwrap(), vec!["poppy", "sam"]);
        assert!(db.set_going(e, "poppy", false).unwrap());
        assert!(db.upcoming_events(0).unwrap()[0].local);
        db.remove_event(e).unwrap();
        assert!(db.going(e).unwrap().is_empty());
    }

    #[test]
    fn stations_are_kept_by_name_whatever_the_case() {
        let db = Db::open(":memory:").unwrap();
        db.set_station("Night Owl FM", "https://owl.example/live", None, "u", 1).unwrap();
        db.set_station("night owl fm", "https://owl.example/new", Some("https://owl.example/guide"), "u", 2).unwrap();
        assert_eq!(db.stations().unwrap(), vec![("night owl fm".into(), "https://owl.example/new".into(), Some("https://owl.example/guide".into()))]);
        assert!(db.remove_station("NIGHT OWL FM").unwrap());
        assert!(db.stations().unwrap().is_empty());
    }

    #[test]
    fn invites_show_whether_theyre_used_and_can_be_revoked() {
        let db = Db::open(":memory:").unwrap();
        db.create_user("plucky-wren-17", "Sam", -1, -1).unwrap();
        db.add_invite("plucky-wren-17", "a", 1_000).unwrap();
        db.add_invite("plucky-wren-17", "b", 2_000).unwrap();
        assert_eq!(db.redeem_invite("a", 500).as_deref(), Some("plucky-wren-17"));
        let list = db.invites(10).unwrap();
        assert_eq!(list[0], ("plucky-wren-17".into(), "Sam".into(), 2_000, None));
        assert_eq!(list[1].3, Some(500));
        assert_eq!(db.revoke_invites("plucky-wren-17").unwrap(), 1, "only the unused one goes");
        assert_eq!(db.redeem_invite("b", 600), None);
    }
}
