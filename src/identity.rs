//! This device's secret key, which makes the names it uses its own. Shared by
//! the terminal client, residents and bridges on the same machine.

use std::io::Read;
use std::path::PathBuf;

fn key_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config").join("world.txt").join("key"))
}

pub fn device_key() -> Option<String> {
    let path = key_path()?;
    if let Ok(k) = std::fs::read_to_string(&path) {
        let k = k.trim().to_string();
        if !k.is_empty() {
            return Some(k);
        }
    }
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom").ok()?.read_exact(&mut bytes).ok()?;
    let key: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::create_dir_all(path.parent()?).ok()?;
    std::fs::write(&path, &key).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Some(key)
}
