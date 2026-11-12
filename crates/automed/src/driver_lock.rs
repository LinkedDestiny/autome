//! One driver at a time per ledger.
//!
//! The desktop app ticks the scheduler every three seconds; the `autome`
//! command ticks it too. Both open the same SQLite file, and SQLite only
//! serialises individual writes — a tick reads a task, decides, launches a
//! session and records it, and two processes interleaving those steps would
//! each see "active, no session" and start the same round twice.
//!
//! So whoever drives holds an advisory `flock` on a file beside the database
//! for the length of one tick or one write command, and releases it after.
//! Holding it per operation rather than for the life of the process is what
//! lets the two front ends coexist: the desktop's tick and a command typed in
//! a terminal take turns instead of one locking the other out.
//!
//! The lock is advisory and local: it says nothing about a database on a
//! network share, which Autome does not support anyway.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A held lock. Dropping it explicitly releases the `flock`.
#[derive(Debug)]
pub struct DriverLock {
    _file: File,
}

impl Drop for DriverLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            // A child between fork and exec can still hold this open file
            // description. Closing our descriptor alone would leave it locked.
            // SAFETY: the descriptor is owned by self and is still open.
            unsafe { libc::flock(self._file.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

/// Where the lock for a ledger lives: beside it, so two installations with two
/// databases — the test suites have one each — never contend. `None` for an
/// in-memory store, which has no second process to exclude.
pub fn lock_path(db_path: &Path) -> Option<PathBuf> {
    let s = db_path.to_string_lossy();
    if s.is_empty() || s == ":memory:" {
        return None;
    }
    Some(PathBuf::from(format!("{s}.driver.lock")))
}

/// Takes the lock if nobody holds it. `Ok(None)` means somebody does.
pub fn try_acquire(db_path: &Path) -> std::io::Result<Option<DriverLock>> {
    let Some(path) = lock_path(db_path) else {
        return Ok(Some(DriverLock {
            _file: null_file()?,
        }));
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        // SAFETY: `flock` on a descriptor we own; it does not outlive `file`.
        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::EWOULDBLOCK) {
                return Ok(None);
            }
            return Err(e);
        }
    }
    Ok(Some(DriverLock { _file: file }))
}

/// Waits up to `timeout` for the lock. A tick can include slow synchronous
/// work such as worktree cleanup, so callers must allow for long contention.
pub fn acquire(db_path: &Path, timeout: Duration) -> std::io::Result<Option<DriverLock>> {
    acquire_with_notice(db_path, timeout, None)
}

/// Calls `notice` once if contention lasts at least two seconds.
pub fn acquire_with_notice(
    db_path: &Path,
    timeout: Duration,
    mut notice: Option<fn()>,
) -> std::io::Result<Option<DriverLock>> {
    let started = Instant::now();
    let deadline = started + timeout;
    loop {
        if let Some(lock) = try_acquire(db_path)? {
            return Ok(Some(lock));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        if started.elapsed() >= Duration::from_secs(2)
            && let Some(notice) = notice.take()
        {
            notice();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A stand-in descriptor for stores with nothing to lock.
fn null_file() -> std::io::Result<File> {
    File::open("/dev/null")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_db(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("automed-lock-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("automed.sqlite3")
    }

    #[test]
    fn a_second_holder_is_refused_until_the_first_lets_go() {
        let db = tmp_db("second");
        let first = try_acquire(&db).unwrap();
        assert!(first.is_some());
        // A second open of the same file is a second open file description,
        // so `flock` excludes it even within one process — which is what lets
        // the suite stand in for two processes.
        assert!(try_acquire(&db).unwrap().is_none());
        drop(first);
        assert!(try_acquire(&db).unwrap().is_some());
    }

    #[test]
    fn dropping_the_holder_releases_the_lock_even_with_an_inherited_descriptor() {
        let db = tmp_db("inherited");
        let first = try_acquire(&db).unwrap().unwrap();
        // dup shares the open file description just like a child between
        // fork and exec, without introducing a timing race into the test.
        let inherited = first._file.try_clone().unwrap();
        assert!(try_acquire(&db).unwrap().is_none());
        drop(first);
        assert!(try_acquire(&db).unwrap().is_some());
        drop(inherited);
    }

    #[test]
    fn waiting_gives_up_at_the_deadline() {
        let db = tmp_db("wait");
        let _held = try_acquire(&db).unwrap().unwrap();
        let started = Instant::now();
        assert!(acquire(&db, Duration::from_millis(120)).unwrap().is_none());
        assert!(started.elapsed() >= Duration::from_millis(120));
    }

    #[test]
    fn prolonged_contention_notifies_once_but_immediate_acquisition_is_silent() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NOTICES: AtomicUsize = AtomicUsize::new(0);
        fn notice() {
            NOTICES.fetch_add(1, Ordering::SeqCst);
        }

        let db = tmp_db("notice");
        let held = acquire_with_notice(&db, Duration::from_secs(3), Some(notice))
            .unwrap()
            .unwrap();
        assert_eq!(NOTICES.load(Ordering::SeqCst), 0);
        assert!(
            acquire_with_notice(&db, Duration::from_millis(2200), Some(notice))
                .unwrap()
                .is_none()
        );
        assert_eq!(NOTICES.load(Ordering::SeqCst), 1);
        drop(held);
        assert!(
            acquire_with_notice(&db, Duration::from_secs(3), Some(notice))
                .unwrap()
                .is_some()
        );
        assert_eq!(NOTICES.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn an_in_memory_store_has_nothing_to_lock() {
        assert_eq!(lock_path(Path::new(":memory:")), None);
        assert!(try_acquire(Path::new(":memory:")).unwrap().is_some());
        assert!(try_acquire(Path::new(":memory:")).unwrap().is_some());
    }
}
