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
        Ok(Db { conn })
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
            .prepare("SELECT id, x, y, name, action, trigger, maker, kind FROM things")?;
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

    pub fn move_thing(&self, id: i64, x: i64, y: i64) -> anyhow::Result<()> {
        self.conn
            .execute("UPDATE things SET x = ?2, y = ?3 WHERE id = ?1", params![id, x, y])?;
        Ok(())
    }

    pub fn remove_thing(&self, id: i64) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM things WHERE id = ?1", [id])?;
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
