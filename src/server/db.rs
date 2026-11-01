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
             CREATE TABLE IF NOT EXISTS door_knocks (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL, key_hash TEXT NOT NULL, code TEXT NOT NULL, at INTEGER NOT NULL);
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
             CREATE TABLE IF NOT EXISTS journal (
                 player TEXT NOT NULL, species TEXT NOT NULL, found_at INTEGER NOT NULL,
                 PRIMARY KEY (player, species));",
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
        self.conn.execute("UPDATE invites SET used = 1 WHERE token_hash = ?1", [token_hash]).ok()?;
        Some(user)
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
    /// The hash of the key that owns a name, if it's been claimed.
    pub fn name_key(&self, name: &str) -> Option<String> {
        self.conn
            .query_row("SELECT key_hash FROM players WHERE name = ?1", [name], |r| r.get(0))
            .ok()
            .flatten()
    }

    pub fn claim_name(&self, name: &str, key_hash: &str, x: i64, y: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO players (name, x, y, tutorial, key_hash) VALUES (?1, ?2, ?3, 0, ?4)
             ON CONFLICT(name) DO UPDATE SET key_hash = excluded.key_hash",
            params![name, x, y, key_hash],
        )?;
        Ok(())
    }

    /// Someone tried a claimed name from a new device: remember the knock and
    /// its code. The same device knocking again within a day keeps its code.
    pub fn add_knock(&self, name: &str, key_hash: &str, new_code: &str) -> anyhow::Result<String> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64;
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT code FROM door_knocks WHERE name = ?1 AND key_hash = ?2 AND at > ?3 - 86400",
                params![name, key_hash, now],
                |r| r.get(0),
            )
            .ok();
        if let Some(code) = existing {
            self.conn.execute(
                "UPDATE door_knocks SET at = ?3 WHERE name = ?1 AND key_hash = ?2",
                params![name, key_hash, now],
            )?;
            return Ok(code);
        }
        self.conn.execute(
            "INSERT INTO door_knocks (name, key_hash, code, at) VALUES (?1, ?2, ?3, ?4)",
            params![name, key_hash, new_code, now],
        )?;
        Ok(new_code.to_string())
    }

    /// Names with recent knocks (within a day): (name, devices knocking, seconds since the latest).
    pub fn knocks(&self) -> anyhow::Result<Vec<(String, i64, i64)>> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64;
        let mut stmt = self.conn.prepare(
            "SELECT name, COUNT(*), ?1 - MAX(at) FROM door_knocks WHERE at > ?1 - 86400
             GROUP BY name ORDER BY MAX(at)",
        )?;
        let rows = stmt.query_map([now], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Let the device that was shown this code use the name from now on (any
    /// other device stops working). Returns the name as stored, if the code matched.
    pub fn welcome(&self, name: &str, code: &str) -> anyhow::Result<Option<String>> {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64;
        let knock: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT name, key_hash FROM door_knocks
                 WHERE name = ?1 COLLATE NOCASE AND code = ?2 COLLATE NOCASE AND at > ?3 - 86400",
                params![name, code.trim(), now],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        let Some((stored, hash)) = knock else { return Ok(None) };
        self.conn.execute("UPDATE players SET key_hash = ?2 WHERE name = ?1", params![stored, hash])?;
        self.conn.execute("DELETE FROM door_knocks WHERE name = ?1", [&stored])?;
        Ok(Some(stored))
    }

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

    /// Free a name so the next device to use it claims it. True if it was claimed.
    pub fn release_name(&self, name: &str) -> anyhow::Result<bool> {
        let n = self.conn.execute(
            "UPDATE players SET key_hash = NULL WHERE name = ?1 COLLATE NOCASE AND key_hash IS NOT NULL",
            [name],
        )?;
        Ok(n > 0)
    }

    pub fn player(&self, name: &str) -> Option<(i64, i64, u8)> {
        self.conn
            .query_row("SELECT x, y, tutorial FROM players WHERE name = ?1", [name], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .ok()
    }

    pub fn save_player(&self, name: &str, x: i64, y: i64, tutorial: u8) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO players (name, x, y, tutorial) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(name) DO UPDATE SET x = excluded.x, y = excluded.y, tutorial = excluded.tutorial",
            params![name, x, y, tutorial],
        )?;
        Ok(())
    }
}
