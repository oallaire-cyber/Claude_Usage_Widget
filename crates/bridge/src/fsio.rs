//! Atomic writes and a best-effort inter-process lock.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Write `bytes` to a temp file in the same directory, flush it to disk, then rename it over `path`.
/// Readers see either the old or the new content, never a partial file. The destination is only
/// touched once the replacement is complete.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        rename_with_retry(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// On Windows a rename can briefly fail while another process holds the destination open without
/// delete sharing (e.g. an antivirus scan). Retry a few times.
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    let mut last = None;
    for _ in 0..10 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Err(last.unwrap_or_else(|| io::Error::other("rename failed")))
}

pub fn append(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(bytes)
}

/// Lock file created with `create_new`. Serialises read-modify-write of `state.json` between
/// concurrent Claude Code sessions. Best effort: after ~300 ms the caller proceeds without it rather
/// than delay Claude Code; a lock older than 5 s is considered abandoned.
pub struct FileLock {
    path: Option<PathBuf>,
}

impl FileLock {
    const ATTEMPTS: u32 = 30;
    const STALE: Duration = Duration::from_secs(5);

    pub fn acquire(path: &Path) -> FileLock {
        for _ in 0..Self::ATTEMPTS {
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(_) => {
                    return FileLock {
                        path: Some(path.to_path_buf()),
                    }
                }
                Err(_) => {
                    let stale = fs::metadata(path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| SystemTime::now().duration_since(t).ok())
                        .is_some_and(|age| age > Self::STALE);
                    if stale {
                        let _ = fs::remove_file(path);
                        continue;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
        FileLock { path: None }
    }

    pub fn held(&self) -> bool {
        self.path.is_some()
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        if let Some(p) = &self.path {
            let _ = fs::remove_file(p);
        }
    }
}
