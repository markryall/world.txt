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
                 name TEXT PRIMARY KEY, x INTEGER NOT NULL, y INTEGER NOT NULL);",
        )?;
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
            .prepare("SELECT id, x, y, name, action, trigger, maker FROM things")?;
        let rows = stmt.query_map([], |r| {
            Ok(ThingRow {
                id: r.get(0)?,
                x: r.get(1)?,
                y: r.get(2)?,
                name: r.get(3)?,
                action: r.get(4)?,
                trigger: r.get(5)?,
                maker: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn add_thing(&self, t: &ThingRow) -> anyhow::Result<i64> {
        self.conn.execute(
            "INSERT INTO things (x, y, name, action, trigger, maker) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![t.x, t.y, t.name, t.action, t.trigger, t.maker],
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

    pub fn player_pos(&self, name: &str) -> Option<(i64, i64)> {
        self.conn
            .query_row("SELECT x, y FROM players WHERE name = ?1", [name], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .ok()
    }

    pub fn save_player(&self, name: &str, x: i64, y: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO players (name, x, y) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET x = excluded.x, y = excluded.y",
            params![name, x, y],
        )?;
        Ok(())
    }
}
