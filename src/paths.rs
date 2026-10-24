//! Program directory layout:
//!
//! ```text
//! <program folder>\
//!   infinifan.exe
//!   config\infinifan.json
//!   logs\infinifan.log, infinifan.1.log
//!   drivers\PawnIO_setup.exe
//! ```
//!
//! If the program folder is not writable (e.g. Program Files), config and logs are
//! stored in %APPDATA%\InfiniFan. The drivers folder always sits next to the program.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const APP_NAME: &str = "infinifan";

pub fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from("."))
}

fn appdata_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("InfiniFan")
}

fn writable(dir: &Path) -> bool {
    let probe = dir.join(".infinifan-write-test");
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Root for config and logs.
fn data_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let exe = exe_dir();
        if writable(&exe) { exe } else { appdata_dir() }
    })
}

fn ensure(dir: PathBuf) -> PathBuf {
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn config_file() -> PathBuf {
    ensure(data_root().join("config")).join(format!("{APP_NAME}.json"))
}

pub fn log_dir() -> PathBuf {
    ensure(data_root().join("logs"))
}

pub fn drivers_dir() -> PathBuf {
    exe_dir().join("drivers")
}

/// Older config locations (for one-time migration), newest first; the program used to be
/// called L-Connect RS (`lconnect-rs`).
pub fn legacy_config_files() -> Vec<PathBuf> {
    let old_appdata = appdata_dir().with_file_name("LConnect-RS");
    vec![
        data_root().join("config").join("lconnect-rs.json"),
        old_appdata.join("config").join("lconnect-rs.json"),
        exe_dir().join("lconnect-rs.json"),
        old_appdata.join("config.json"),
    ]
}

/// Opens the log for the current run (`name` = infinifan / infinifan-gui); the previous run is kept as `.1.log`.
pub fn open_log(name: &str) -> Option<std::fs::File> {
    let dir = log_dir();
    let cur = dir.join(format!("{name}.log"));
    let prev = dir.join(format!("{name}.1.log"));
    if cur.exists() {
        let _ = std::fs::rename(&cur, &prev);
    }
    // Clean up the log from an older version that put it right next to the exe.
    let _ = std::fs::remove_file(exe_dir().join("lconnect-rs.log"));
    std::fs::File::create(cur).ok()
}
