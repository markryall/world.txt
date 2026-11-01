//! Daily copies of the world, made with SQLite's online backup so they're
//! consistent even while people are playing.

use std::path::{Path, PathBuf};
use std::time::Duration;

fn backup_dir(db_path: &str) -> PathBuf {
    Path::new(db_path).parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")).join("backups")
}

fn stem(db_path: &str) -> String {
    Path::new(db_path).file_stem().and_then(|s| s.to_str()).unwrap_or("world").to_string()
}

/// Copy the world to backups/<name>-<date>.db, unless today's copy exists
/// (or `force`), then keep only the newest `keep`. Returns the new copy's path.
pub fn backup(db_path: &str, keep: usize, force: bool) -> anyhow::Result<Option<PathBuf>> {
    let dir = backup_dir(db_path);
    std::fs::create_dir_all(&dir)?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let target = dir.join(format!("{}-{today}.db", stem(db_path)));
    if target.exists() && !force {
        return Ok(None);
    }
    let src = rusqlite::Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let tmp = target.with_extension("db.partial");
    src.backup(rusqlite::MAIN_DB, &tmp, None::<fn(rusqlite::backup::Progress)>)?;
    std::fs::rename(&tmp, &target)?;
    prune(&dir, &stem(db_path), keep)?;
    Ok(Some(target))
}

fn prune(dir: &Path, stem: &str, keep: usize) -> anyhow::Result<()> {
    let prefix = format!("{stem}-");
    let mut copies: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|x| x == "db")
                && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix))
        })
        .collect();
    copies.sort(); // dates sort as text
    let excess = copies.len().saturating_sub(keep.max(1));
    for old in &copies[..excess] {
        std::fs::remove_file(old)?;
    }
    Ok(())
}

/// Keep a copy for every day the world is running.
pub async fn run_daily(db_path: String, keep: usize) {
    loop {
        let path = db_path.clone();
        match tokio::task::spawn_blocking(move || backup(&path, keep, false)).await {
            Ok(Ok(Some(p))) => println!("💾 backed up the world to {}", p.display()),
            Ok(Ok(None)) => {}
            Ok(Err(e)) => eprintln!("⚠ backup failed: {e}"),
            Err(e) => eprintln!("⚠ backup task failed: {e}"),
        }
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_once_a_day_and_keeps_the_newest() {
        let dir = std::env::temp_dir().join(format!("world-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("backups")).unwrap();
        let db = dir.join("island.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE t (x); INSERT INTO t VALUES (42);").unwrap();
        for day in ["2026-10-01", "2026-10-02", "2026-10-03"] {
            std::fs::write(dir.join("backups").join(format!("island-{day}.db")), "old").unwrap();
        }
        let db_path = db.to_str().unwrap();

        let made = backup(db_path, 2, false).unwrap().expect("first backup today");
        assert!(backup(db_path, 2, false).unwrap().is_none(), "only one a day");
        let copy = rusqlite::Connection::open(&made).unwrap();
        let x: i64 = copy.query_row("SELECT x FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(x, 42);
        let left: Vec<_> = std::fs::read_dir(dir.join("backups")).unwrap().collect();
        assert_eq!(left.len(), 2, "kept the newest two");
        assert!(!dir.join("backups/island-2026-10-01.db").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
