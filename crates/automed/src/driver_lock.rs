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

/// A process-lifetime registration, separate from the per-operation driver lock.
/// File::open/OpenOptions use O_CLOEXEC; exec'd sessions cannot keep it alive.
pub struct Registration {
    file: Option<File>,
}
impl Drop for Registration {
    fn drop(&mut self) {
        if let Some(file) = &self.file {
            use std::os::unix::io::AsRawFd;
            // Closing alone leaves the lock held while a concurrent child is
            // between fork and exec, even though the descriptor is CLOEXEC.
            // SAFETY: self owns the descriptor and it is still open.
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}
impl Registration {
    pub fn touch(&self) -> std::io::Result<()> {
        if let Some(file) = &self.file {
            file.set_modified(std::time::SystemTime::now())?;
        }
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Record {
    pid: u32,
    command: String,
    started_at: String,
    interval_secs: Option<f64>,
}
#[derive(Debug, serde::Serialize)]
pub struct DriverInfo {
    pub pid: u32,
    pub command: String,
    pub started_at: String,
    pub last_tick_at: String,
    pub driving: bool,
}
fn registration_dir(db: &Path) -> Option<PathBuf> {
    lock_path(db).map(|_| PathBuf::from(format!("{}.drivers", db.display())))
}

pub fn register(
    db: &Path,
    command: &str,
    interval_secs: Option<f64>,
) -> std::io::Result<Registration> {
    use std::io::Write;
    use std::os::unix::{fs::OpenOptionsExt, io::AsRawFd};
    let Some(dir) = registration_dir(db) else {
        return Ok(Registration { file: None });
    };
    if interval_secs.is_some_and(|n| !n.is_finite() || n < 0.0) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid driver interval",
        ));
    }
    std::fs::create_dir_all(&dir)?;
    let pid = std::process::id();
    let temp = dir.join(format!(".{pid}-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        let record = Record {
            pid,
            command: command.chars().take(120).collect(),
            started_at: crate::store::now_iso(),
            interval_secs,
        };
        serde_json::to_writer(&mut file, &record)?;
        file.flush()?;
        // SAFETY: file owns this descriptor throughout the registration.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        std::fs::rename(&temp, dir.join(format!("{pid}.json")))?;
        Ok(Registration { file: Some(file) })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}

pub fn drivers(db: &Path) -> std::io::Result<Vec<DriverInfo>> {
    use std::os::unix::{fs::MetadataExt, io::AsRawFd};
    let Some(dir) = registration_dir(db) else {
        return Ok(vec![]);
    };
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    let mut result = vec![];
    for entry in entries {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.')
            || entry.path().extension().is_none_or(|s| s != "json")
        {
            continue;
        }
        let file = match File::open(entry.path()) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        // SAFETY: file is a live descriptor; nonblocking shared lock probes ownership.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) } == 0 {
            // Another reader may already have removed this stale entry.
            if let Ok(current) = std::fs::metadata(entry.path())
                && current.ino() == file.metadata()?.ino()
            {
                match std::fs::remove_file(entry.path()) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
            }
            continue;
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EWOULDBLOCK) {
            return Err(error);
        }
        let record: Record = serde_json::from_reader(&file)?;
        let modified = file.metadata()?.modified()?;
        let age = std::time::SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default()
            .as_secs_f64();
        let last_tick_at = crate::store::format_iso(
            modified
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        );
        result.push(DriverInfo {
            pid: record.pid,
            command: record.command,
            started_at: record.started_at,
            last_tick_at,
            driving: is_driving(age, record.interval_secs),
        });
    }
    result.sort_by_key(|d| d.pid);
    Ok(result)
}

fn is_driving(age: f64, interval: Option<f64>) -> bool {
    age <= (3.0 * interval.unwrap_or(0.0)).max(30.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_is_visible_with_bounded_unicode_command() {
        use std::os::unix::io::AsRawFd;
        let db = tmp_db("registered");
        let registration = register(&db, &"驱".repeat(121), Some(1.0)).unwrap();
        let info = drivers(&db).unwrap();
        assert_eq!(info.len(), 1);
        assert_eq!(info[0].pid, std::process::id());
        assert_eq!(info[0].command.chars().count(), 120);
        assert!(info[0].driving);
        // SAFETY: the owned registration descriptor is valid.
        let flags = unsafe {
            libc::fcntl(
                registration.file.as_ref().unwrap().as_raw_fd(),
                libc::F_GETFD,
            )
        };
        assert_ne!(flags & libc::FD_CLOEXEC, 0);
        for interval in [f64::NAN, f64::INFINITY, -1.0] {
            assert!(register(&db, "", Some(interval)).is_err());
        }
    }

    #[test]
    fn released_registration_is_removed() {
        let db = tmp_db("released");
        let registration = register(&db, "autome run", None).unwrap();
        // Model a concurrent child between fork and exec: CLOEXEC has not
        // closed its inherited reference to the same open file description.
        let inherited = registration.file.as_ref().unwrap().try_clone().unwrap();
        assert_eq!(drivers(&db).unwrap().len(), 1);
        drop(registration);
        assert!(drivers(&db).unwrap().is_empty());
        assert_eq!(
            std::fs::read_dir(registration_dir(&db).unwrap())
                .unwrap()
                .count(),
            0
        );
        drop(inherited);
    }

    #[test]
    fn pid_reuse_does_not_keep_stale_registration_alive() {
        let db = tmp_db("reused");
        let registration = register(&db, "old process with our live pid", None).unwrap();
        drop(registration);
        // Our PID is alive, but the old inode no longer has a holder.
        assert!(drivers(&db).unwrap().is_empty());
    }

    #[test]
    fn memory_registration_has_no_file() {
        for path in ["", ":memory:"] {
            let registration = register(Path::new(path), "", None).unwrap();
            assert!(registration.file.is_none());
            registration.touch().unwrap();
            assert!(drivers(Path::new(path)).unwrap().is_empty());
        }
    }

    #[test]
    fn touch_updates_timestamp_and_driving_expires() {
        assert!(is_driving(30.0, None));
        assert!(!is_driving(30.001, None));
        assert!(is_driving(60.0, Some(20.0)));
        assert!(!is_driving(60.001, Some(20.0)));
        assert!(is_driving(0.0, Some(f64::MAX)));
        let db = tmp_db("heartbeat");
        for (interval, age, expected) in [
            (None, 29, true),
            (None, 31, false),
            (Some(20.0), 59, true),
            (Some(20.0), 61, false),
            (Some(0.0), 31, false),
        ] {
            let registration = register(&db, "autome core", interval).unwrap();
            registration
                .file
                .as_ref()
                .unwrap()
                .set_modified(std::time::SystemTime::now() - Duration::from_secs(age))
                .unwrap();
            let before = drivers(&db).unwrap().remove(0);
            assert_eq!(before.driving, expected);
            registration.touch().unwrap();
            let after = drivers(&db).unwrap().remove(0);
            assert!(after.driving);
            assert!(after.last_tick_at > before.last_tick_at);
        }
    }

    #[test]
    fn unpublished_hidden_files_are_ignored() {
        let db = tmp_db("hidden");
        let dir = registration_dir(&db).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".publishing.json");
        std::fs::write(&path, "incomplete json").unwrap();
        assert!(drivers(&db).unwrap().is_empty());
        assert!(path.exists());
        // Missing directories are empty; filesystem errors are not hidden as empty.
        let invalid = tmp_db("bad-dir");
        std::fs::write(registration_dir(&invalid).unwrap(), "not a directory").unwrap();
        assert!(drivers(&invalid).is_err());
    }

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
