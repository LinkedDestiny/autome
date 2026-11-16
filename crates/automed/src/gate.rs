//! The project's gate, run by the core itself.
//!
//! A round that says "the tests pass" is making a claim; this is where the
//! core stops taking the claim and checks. The project names one command in
//! `[gate]`, and the core runs it on the exact commit a round left behind —
//! after each implementation round, again on the rebased commit before the
//! merge, and once more on the merge commit after it. The result is bound to
//! that commit: a later round cannot inherit it, and a gate whose commit moved
//! while it ran is thrown away.
//!
//! The command takes minutes, and the core's stdio loop is serial — a gate run
//! inline would freeze every request the desktop sends behind it. So a gate is
//! a background job shaped like a session: a small wrapper started in its own
//! process group writes a pid, tees the command's output to a log and, however
//! it ends, writes an exit marker in the session wrapper's format. Each tick
//! looks at the files. A core restarted mid-gate finds them where it left
//! them.
//!
//! Files live under `.autome/output/gates/<task>/`, named for the gate node
//! and the commit, so the same commit is never gated twice by accident and a
//! person can open the log the panel points at.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use autome_domain::session::ExitMarker;

/// The wrapper. Arguments travel in the environment rather than the command
/// line so that nothing here needs quoting — the command is the user's, and
/// the paths can contain anything a directory name can.
const WRAPPER: &str = r#"
echo $$ > "$AUTOME_GATE_PID"
code_file="$AUTOME_GATE_EXIT.code"
rm -f "$code_file"
{ sh -c "$AUTOME_GATE_COMMAND" < /dev/null 2>&1; echo $? > "$code_file"; } > "$AUTOME_GATE_LOG"
code=$(cat "$code_file" 2>/dev/null || echo 70)
rm -f "$code_file"
{
  echo "exit_code=$code"
  echo "ended_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "marker_end"
} > "$AUTOME_GATE_EXIT.tmp" 2>/dev/null
mv -f "$AUTOME_GATE_EXIT.tmp" "$AUTOME_GATE_EXIT" 2>/dev/null
rm -f "$AUTOME_GATE_PID"
"#;

/// Exit code recorded for a gate the core killed for running too long — the
/// same number `timeout(1)` uses.
pub const TIMED_OUT: i32 = 124;

/// Launch facts. Quick runs retain their command and timeout across config edits.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Launch {
    Quick {
        command: String,
        timeout_minutes: u32,
    },
    Full,
    Repro,
}

impl Launch {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Quick { .. } => "quick",
            Self::Full => "full",
            Self::Repro => "repro",
        }
    }

    pub fn timeout(&self, fallback: u32) -> u32 {
        match self {
            Self::Quick {
                timeout_minutes, ..
            } => *timeout_minutes,
            _ => fallback,
        }
    }
}

/// The files one gate run owns.
#[derive(Debug, Clone)]
pub struct Job {
    pub log: PathBuf,
    pub pid: PathBuf,
    pub exit: PathBuf,
    pub started: PathBuf,
    pub passes: PathBuf,
    pub report: PathBuf,
}

impl Job {
    /// `<repo>/.autome/output/gates/<task>/<node>-<key>.*`
    pub fn at(repo: &Path, task_id: &str, node: &str, key: &str) -> Job {
        let dir = dir_for(repo, task_id);
        let stem = format!("{node}-{key}");
        Job {
            log: dir.join(format!("{stem}.log")),
            pid: dir.join(format!("{stem}.pid")),
            exit: dir.join(format!("{stem}.exit")),
            started: dir.join(format!("{stem}.started")),
            passes: dir.join(format!("{stem}.passes")),
            report: dir.join(format!("{stem}.report")),
        }
    }

    /// Persists that this FixGate started without the project gate. This
    /// survives clear/start; the scheduler replaces it before every launch.
    pub fn set_repro_only(&self, value: bool) -> std::io::Result<()> {
        let path = self.started.with_extension("repro-only");
        if value {
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(path, "")
        } else {
            match std::fs::remove_file(path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                result => result,
            }
        }
    }

    pub fn repro_only(&self) -> bool {
        self.started.with_extension("repro-only").exists()
    }

    pub fn set_launch(&self, launch: &Launch) -> std::io::Result<()> {
        std::fs::create_dir_all(self.started.parent().unwrap())?;
        std::fs::write(
            self.started.with_extension("kind"),
            serde_json::to_vec(launch)?,
        )
    }

    pub fn launch(&self) -> Option<Launch> {
        serde_json::from_slice(&std::fs::read(self.started.with_extension("kind")).ok()?).ok()
    }

    /// Whether this job was ever started (and not cleared since).
    pub fn exists(&self) -> bool {
        self.started.exists()
    }

    /// Throws the run away: the commit it was for is not the one that
    /// matters any more.
    pub fn clear(&self) {
        for p in [
            &self.log,
            &self.pid,
            &self.exit,
            &self.started,
            &self.passes,
            &self.report,
        ] {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Reports are optional. Invalid or unreadable reports add no event fields.
pub fn read_report(job: &Job) -> Option<serde_json::Value> {
    use std::io::Read;
    const LIMIT: u64 = 256 * 1024;
    let meta = std::fs::metadata(&job.report).ok()?;
    if !meta.is_file() || meta.len() > LIMIT {
        return None;
    }
    let mut text = String::new();
    std::fs::File::open(&job.report)
        .ok()?
        .take(LIMIT + 1)
        .read_to_string(&mut text)
        .ok()?;
    if text.len() as u64 > LIMIT {
        return None;
    }
    let snapshot = std::fs::read_to_string(&job.passes).unwrap_or_default();
    let keys: std::collections::HashSet<_> = snapshot
        .lines()
        .filter_map(|line| line.split_once('\t').map(|(key, _)| key))
        .collect();
    let mut lines = text.lines();
    if lines.next()? != "autome-gate-report 1" {
        return None;
    }
    let mut steps = serde_json::json!({"ran":0,"reused":0,"failed":0,"not_run":0,"unverified":0});
    let mut passes = Vec::new();
    for line in lines {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 || fields[2].is_empty() || fields[2].contains('\r') {
            return None;
        }
        let (status, key, step) = (fields[0], fields[1], fields[2]);
        if !(valid_key(key) || key == "-" && matches!(status, "partial" | "not_run" | "failed")) {
            return None;
        }
        let counter = match status {
            "ran" | "reused" | "failed" | "not_run" => status,
            "partial" => "ran",
            _ => return None,
        };
        steps[counter] = serde_json::json!(steps[counter].as_u64()? + 1);
        if status == "failed" {
            steps["ran"] = serde_json::json!(steps["ran"].as_u64()? + 1);
        }
        if status == "ran" {
            passes.push(serde_json::json!({"step":step,"key":key}));
        }
        if status == "reused" && !keys.contains(key) {
            steps["unverified"] = serde_json::json!(steps["unverified"].as_u64()? + 1);
        }
    }
    Some(serde_json::json!({"steps":steps,"passes":passes}))
}

pub(crate) fn valid_key(key: &str) -> bool {
    (40..=64).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

pub fn dir_for(repo: &Path, task_id: &str) -> PathBuf {
    repo.join(".autome/output/gates").join(task_id)
}

/// What a gate job is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Poll {
    Running,
    Done {
        exit_code: i32,
    },
    /// No marker and no process: the machine slept through it, or something
    /// killed the wrapper. Nothing about the code is known, so the job is
    /// run again rather than counted as a failure.
    Vanished,
}

/// Starts a gate run in the background.
pub fn start(job: &Job, cwd: &Path, command: &str, env: &[(&str, &str)]) -> std::io::Result<()> {
    start_with_passes(job, cwd, command, env, "")
}

/// Snapshot is written after clearing stale job files and before spawning the command.
pub fn start_with_passes(
    job: &Job,
    cwd: &Path,
    command: &str,
    env: &[(&str, &str)],
    passes: &str,
) -> std::io::Result<()> {
    if let Some(dir) = job.log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    job.clear();
    std::fs::write(&job.passes, passes)?;
    std::fs::write(&job.started, now_secs().to_string())?;

    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(WRAPPER)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env_remove("AUTOME_ROUND_START")
        .env_remove("AUTOME_BASE")
        .env("AUTOME_GATE_COMMAND", command)
        .env("AUTOME_GATE_LOG", &job.log)
        .env("AUTOME_GATE_PID", &job.pid)
        .env("AUTOME_GATE_EXIT", &job.exit)
        .env("AUTOME_GATE_PASSES", &job.passes)
        .env("AUTOME_GATE_REPORT", &job.report);
    for (k, v) in env {
        cmd.env(k, v);
    }
    crate::launcher::strip_git_env(&mut cmd);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own group, so a timeout can take down whatever the command
        // started (a dev server, a test runner's workers) and so the driver's
        // Ctrl-C does not reach it.
        cmd.process_group(0);
    }
    let mut child = cmd.spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Looks at a started job. Kills it if it has run past `timeout_minutes`.
pub fn poll(job: &Job, timeout_minutes: u32) -> Poll {
    poll_as(job, timeout_minutes, "门禁")
}

pub fn poll_as(job: &Job, timeout_minutes: u32, what: &str) -> Poll {
    let completed = || {
        std::fs::read_to_string(&job.exit)
            .ok()
            .and_then(|t| ExitMarker::parse(&t))
            .map(|marker| Poll::Done {
                exit_code: marker.exit_code,
            })
    };
    if let Some(done) = completed() {
        return done;
    }
    let pid = std::fs::read_to_string(&job.pid)
        .ok()
        .and_then(|t| t.trim().parse::<i32>().ok());
    let started = std::fs::read_to_string(&job.started)
        .ok()
        .and_then(|t| t.trim().parse::<u64>().ok())
        .unwrap_or_else(now_secs);
    let elapsed = now_secs().saturating_sub(started);

    match pid {
        Some(pid) if crate::launcher::pid_alive(pid) => {
            if elapsed >= u64::from(timeout_minutes) * 60 {
                kill_group(pid);
                let note =
                    format!("\n=== Autome：{what}运行超过 {timeout_minutes} 分钟，已终止 ===\n");
                append(&job.log, &note);
                write_marker(&job.exit, TIMED_OUT);
                return Poll::Done {
                    exit_code: TIMED_OUT,
                };
            }
            Poll::Running
        }
        // The pid file is written by the wrapper's first line; a job started
        // a moment ago may not have one yet.
        None if elapsed < 10 => Poll::Running,
        // The wrapper may publish its result and exit between the first
        // marker read and the liveness check. Collect it instead of rerunning.
        _ => completed().unwrap_or(Poll::Vanished),
    }
}

/// Read-only liveness, including the wrapper startup window.
pub fn running(job: &Job) -> bool {
    if !job.exists()
        || std::fs::read_to_string(&job.exit)
            .ok()
            .and_then(|text| ExitMarker::parse(&text))
            .is_some()
    {
        return false;
    }
    match std::fs::read_to_string(&job.pid)
        .ok()
        .and_then(|text| text.trim().parse::<i32>().ok())
    {
        Some(pid) => crate::launcher::pid_alive(pid),
        None => std::fs::read_to_string(&job.started)
            .ok()
            .and_then(|text| text.trim().parse::<u64>().ok())
            .is_some_and(|started| now_secs().saturating_sub(started) < 10),
    }
}

/// Stops every gate run of one task: the task was stopped or cancelled, and a
/// gate nobody will read must not keep a test suite running.
pub fn stop_all(repo: &Path, task_id: &str) {
    let Ok(entries) = std::fs::read_dir(dir_for(repo, task_id)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pid") {
            continue;
        }
        if let Some(pid) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| t.trim().parse::<i32>().ok())
            && crate::launcher::pid_alive(pid)
        {
            kill_group(pid);
        }
    }
}

/// Remove only this node's jobs, after stopping any surviving wrapper.
pub fn discard_node(repo: &Path, task_id: &str, node: &str) -> std::io::Result<()> {
    discard_except(repo, task_id, node, None)
}

pub fn discard_stale(
    repo: &Path,
    task_id: &str,
    node: &str,
    keep_key: &str,
) -> std::io::Result<()> {
    discard_except(repo, task_id, node, Some(keep_key))
}

fn discard_except(
    repo: &Path,
    task_id: &str,
    node: &str,
    keep: Option<&str>,
) -> std::io::Result<()> {
    let dir = dir_for(repo, task_id);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let prefix = format!("{node}-");
    let mut stems = std::collections::BTreeSet::new();
    for entry in entries {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(&prefix) {
            continue;
        }
        for suffix in [
            ".started",
            ".pid",
            ".exit",
            ".log",
            ".exit.tmp",
            ".exit.code",
            ".repro-only",
            ".kind",
            ".passes",
            ".report",
        ] {
            if let Some(stem) = name.strip_suffix(suffix) {
                if keep.is_none_or(|key| stem != format!("{node}-{key}")) {
                    stems.insert(stem.to_string());
                }
                break;
            }
        }
    }
    for stem in &stems {
        let pid_path = dir.join(format!("{stem}.pid"));
        let started = dir.join(format!("{stem}.started"));
        let exit = dir.join(format!("{stem}.exit"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while started.exists()
            && !pid_path.exists()
            && !exit.exists()
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if let Some(pid) = std::fs::read_to_string(&pid_path)
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
            && crate::launcher::pid_alive(pid)
        {
            kill_group(pid);
        }
    }
    for stem in stems {
        for suffix in [
            "started",
            "pid",
            "exit",
            "log",
            "exit.tmp",
            "exit.code",
            "repro-only",
            "kind",
            "passes",
            "report",
        ] {
            match std::fs::remove_file(dir.join(format!("{stem}.{suffix}"))) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

/// The fingerprint window: all complete lines from the last 256 KiB.
pub fn log_window(log: &Path) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let read = || -> std::io::Result<String> {
        let mut file = std::fs::File::open(log)?;
        let offset = file.metadata()?.len().saturating_sub(256 * 1024);
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = Vec::new();
        file.take(256 * 1024).read_to_end(&mut bytes)?;
        let text = String::from_utf8_lossy(&bytes);
        let text = if offset > 0 {
            text.split_once('\n').map_or("", |(_, rest)| rest)
        } else {
            &text
        };
        Ok(text.lines().collect::<Vec<_>>().join("\n"))
    };
    read().unwrap_or_default()
}

/// Closed digest segments from the fingerprint window, bounded on UTF-8 boundaries.
pub fn digest(log: &Path) -> Option<String> {
    let text = log_window(log);
    let mut sections = Vec::new();
    let mut pending = None;
    for line in text.lines() {
        if line.starts_with("autome-gate-digest begin ") {
            pending = Some(Vec::new());
        } else if line == "autome-gate-digest end" {
            if let Some(lines) = pending.take() {
                sections.push(lines.join("\n"));
            }
        } else if let Some(lines) = &mut pending {
            lines.push(line);
        }
    }
    if sections.is_empty() {
        return None;
    }
    let mut text = sections.join("\n");
    const MAX: usize = 16 * 1024;
    if text.len() > MAX {
        let note = format!("\n…完整内容见 `{}` 的 autome-gate-digest 段", log.display());
        let mut end = MAX.saturating_sub(note.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str(&note);
    }
    Some(text)
}

/// Last non-marker lines for the prompt and panel, bounded to 6000 bytes.
pub fn tail(log: &Path, lines: usize) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let mut in_segment = false;
    let all: Vec<&str> = text
        .lines()
        .filter(|line| {
            if line.starts_with("autome-gate-digest begin ")
                || line.starts_with("autome-gate-cases begin ")
            {
                in_segment = true;
                return false;
            }
            if *line == "autome-gate-digest end" || *line == "autome-gate-cases end" {
                in_segment = false;
                return false;
            }
            !in_segment
        })
        .collect();
    let start = all.len().saturating_sub(lines);
    let mut out = all[start..].join("\n");
    const MAX: usize = 6000;
    if out.len() > MAX {
        let mut cut = out.len() - MAX;
        while !out.is_char_boundary(cut) {
            cut += 1;
        }
        out = format!("…{}", &out[cut..]);
    }
    out
}

fn kill_group(pid: i32) {
    if pid <= 1 {
        return;
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(-pid, libc::SIGTERM);
        std::thread::sleep(std::time::Duration::from_millis(500));
        if libc::kill(-pid, 0) == 0 {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
}

fn write_marker(path: &Path, code: i32) {
    let marker = ExitMarker {
        exit_code: code,
        ended_at: crate::store::now_iso(),
    };
    let tmp = path.with_extension("exit.tmp");
    if std::fs::write(&tmp, marker.render()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

fn append(path: &Path, text: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
    {
        let _ = f.write_all(text.as_bytes());
    }
}

pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    struct DigestDir(std::path::PathBuf);
    impl Drop for DigestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn digest_file(text: &str) -> (DigestDir, std::path::PathBuf) {
        let dir = DigestDir(sandbox(&format!("digest-{}", uuid::Uuid::new_v4())));
        let path = dir.0.join("gate.log");
        std::fs::write(&path, text).unwrap();
        (dir, path)
    }
    #[test]
    fn digest_single() {
        let (_dir, path) = digest_file(
            "before\nautome-gate-digest begin Web e2e\n失败用例：a\nautome-gate-digest end\nafter",
        );
        assert_eq!(super::digest(&path).as_deref(), Some("失败用例：a"));
        assert_eq!(super::tail(&path, 60), "before\nafter");
    }
    #[test]
    fn digest_multiple() {
        let (_dir, path) = digest_file(
            "autome-gate-digest begin a\none\nautome-gate-digest end\nautome-gate-digest begin b\ntwo\nautome-gate-digest end",
        );
        assert_eq!(super::digest(&path).as_deref(), Some("one\ntwo"));
    }
    #[test]
    fn digest_unclosed() {
        let (_dir, path) = digest_file("autome-gate-digest begin a\none");
        assert_eq!(super::digest(&path), None);
        std::fs::write(
            &path,
            format!(
                "autome-gate-digest begin a\n{}\nautome-gate-digest end",
                "x".repeat(256 * 1024)
            ),
        )
        .unwrap();
        assert_eq!(super::digest(&path), None);
    }
    #[test]
    fn digest_utf8_limit() {
        let (_dir, path) = digest_file(&format!(
            "autome-gate-digest begin a\n{}\nautome-gate-digest end",
            "汉".repeat(6000)
        ));
        let text = super::digest(&path).unwrap();
        assert!(text.len() <= 16 * 1024);
        assert!(!text.contains('�'));
        assert!(text.contains(&path.display().to_string()));
        assert!(text.contains("完整内容见"));
    }
    #[test]
    fn digest_absent() {
        let (_dir, path) = digest_file("ordinary log");
        assert_eq!(super::digest(&path), None);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(super::digest(&path), None);
    }

    #[test]
    fn same_failure_log_window() {
        let dir = sandbox("same-failure-window");
        let log = dir.join("log");
        assert_eq!(log_window(&log), "");
        std::fs::write(&log, "").unwrap();
        assert_eq!(log_window(&log), "");
        std::fs::write(&log, "单行").unwrap();
        assert_eq!(log_window(&log), "单行");
        let lines = (0..500).map(|i| format!("line {i}")).collect::<Vec<_>>();
        std::fs::write(&log, lines.join("\n")).unwrap();
        assert_eq!(log_window(&log), lines.join("\n"));
        let failure = |case| {
            std::fs::write(
                &log,
                format!("==> Rust\ntest {case} ... FAILED\nFAILED (1s)\n==> Web\n{}\nError: unrelated\nFAILED Rust\n", lines.join("\n")),
            )
            .unwrap();
            autome_domain::gate_failure::fingerprint(&log_window(&log), 1, false)
        };
        let first = failure("alpha");
        assert_eq!(first.summary, "Rust：test alpha ... FAILED");
        assert_ne!(first.id, failure("beta").id);
        std::fs::write(&log, format!("{}\nlast\n", "中".repeat(100_000))).unwrap();
        assert_eq!(log_window(&log), "last");
        std::fs::write(&log, "x".repeat(300_000)).unwrap();
        assert_eq!(log_window(&log), "");
        assert_eq!(log_window(&dir), "");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn gate_timeout_real_poll_tail_matches_domain() {
        let dir = sandbox("timeout-domain");
        let job = Job::at(&dir, "T-1", "gate", "abc");
        start(&job, &dir, "sleep 30", &[]).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !job.pid.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(job.pid.exists());
        std::fs::write(&job.started, (now_secs() - 3600).to_string()).unwrap();
        assert_eq!(
            poll_as(&job, 1, "门禁"),
            Poll::Done {
                exit_code: TIMED_OUT
            }
        );
        assert_eq!(
            autome_domain::gate_failure::kernel_timeout(&tail(&job.log, 60)),
            Some(1)
        );
    }

    use super::*;

    #[test]
    fn gate_report_counts_only_complete_passes() {
        let dir = sandbox("report-counts");
        let job = Job::at(&dir, "T", "gate", "a");
        std::fs::create_dir_all(job.report.parent().unwrap()).unwrap();
        let key = "a".repeat(40);
        std::fs::write(&job.passes, format!("{key}\tT\tgate\thead\ttime\n")).unwrap();
        std::fs::write(&job.report, format!("autome-gate-report 1\nran\t{key}\tFormat\nreused\t{key}\tClippy\nfailed\t-\tTest\npartial\t-\tRelated\nnot_run\t-\tRemaining\n")).unwrap();
        let report = read_report(&job).unwrap();
        assert_eq!(
            report["steps"],
            serde_json::json!({"ran":3,"reused":1,"failed":1,"not_run":1,"unverified":0})
        );
        assert_eq!(
            report["passes"],
            serde_json::json!([{"step":"Format","key":key}])
        );
        // Completed steps survive a failing or timed-out command too.
        for (suffix, code) in [("exit 0", 0), ("exit 7", 7), ("sleep 30", TIMED_OUT)] {
            let command = format!(
                "printf 'autome-gate-report 1\nran\t{key}\tCheck\n' > \"$AUTOME_GATE_REPORT\"; {suffix}"
            );
            start_with_passes(&job, &dir, &command, &[], "").unwrap();
            if code == TIMED_OUT {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while read_report(&job).is_none() && std::time::Instant::now() < deadline {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                assert!(read_report(&job).is_some());
                std::fs::write(&job.started, (now_secs() - 3600).to_string()).unwrap();
                assert_eq!(poll(&job, 1), Poll::Done { exit_code: code });
            } else {
                assert_eq!(wait_done(&job, 5), Poll::Done { exit_code: code });
            }
            assert_eq!(
                read_report(&job).unwrap()["passes"],
                serde_json::json!([{"step":"Check","key":key}])
            );
        }

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn gate_report_invalid_missing_unreadable_and_size_boundary() {
        let dir = sandbox("report-invalid");
        let job = Job::at(&dir, "T", "gate", "a");
        assert!(read_report(&job).is_none());
        std::fs::create_dir_all(job.report.parent().unwrap()).unwrap();
        std::fs::create_dir(&job.report).unwrap();
        assert!(read_report(&job).is_none());
        std::fs::remove_dir(&job.report).unwrap();
        for bad in [
            "",
            "autome-gate-report 2\n",
            "autome-gate-report 1\nunknown\t-\tx\n",
            "autome-gate-report 1\nran\t-\tx\n",
            "autome-gate-report 1\npartial\t-\t\n",
            "autome-gate-report 1\npartial\t-\tx\textra\n",
        ] {
            std::fs::write(&job.report, bad).unwrap();
            assert!(read_report(&job).is_none(), "{bad:?}");
        }
        let prefix = format!("autome-gate-report 1\nran\t{}\t", "a".repeat(64));
        let text = format!("{}{}\n", prefix, "x".repeat(256 * 1024 - prefix.len() - 1));
        std::fs::write(&job.report, &text).unwrap();
        assert!(read_report(&job).is_some());
        std::fs::write(&job.report, format!("{text}x")).unwrap();
        assert!(read_report(&job).is_none());
        std::fs::write(&job.report, "autome-gate-report 1\n").unwrap();
        assert_eq!(read_report(&job).unwrap()["passes"], serde_json::json!([]));
        for length in [0, 39, 65] {
            assert!(!valid_key(&"a".repeat(length)));
        }
        for length in [40, 64] {
            assert!(valid_key(&"a".repeat(length)));
        }
        assert!(!valid_key(&"G".repeat(40)));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn gate_report_unverified_reuse_is_never_a_pass() {
        let dir = sandbox("report-unverified");
        let job = Job::at(&dir, "T", "gate", "a");
        std::fs::create_dir_all(job.report.parent().unwrap()).unwrap();
        std::fs::write(
            &job.report,
            format!("autome-gate-report 1\nreused\t{}\tCheck\n", "a".repeat(40)),
        )
        .unwrap();
        let report = read_report(&job).unwrap();
        assert_eq!(report["steps"]["unverified"], 1);
        assert_eq!(report["passes"], serde_json::json!([]));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn gate_report_clear_and_discard_clean_sidecars() {
        let dir = sandbox("report-clean");
        for action in ["clear", "stale", "node"] {
            let job = Job::at(&dir, "T", "gate", "old");
            std::fs::create_dir_all(job.report.parent().unwrap()).unwrap();
            std::fs::write(&job.report, "report").unwrap();
            std::fs::write(&job.passes, "passes").unwrap();
            match action {
                "clear" => job.clear(),
                "stale" => discard_stale(&dir, "T", "gate", "new").unwrap(),
                _ => discard_node(&dir, "T", "gate").unwrap(),
            }
            assert!(!job.report.exists());
            assert!(!job.passes.exists());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn sandbox(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("automed-gate-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn before_merge_repro_marker_survives_start_and_clear_but_not_discard() {
        let repo = sandbox("repro-marker");
        let job = Job::at(&repo, "T-1", "fix_gate", "abc");
        assert!(!job.repro_only());
        job.set_repro_only(false).unwrap();
        job.set_repro_only(true).unwrap();
        assert!(job.repro_only());
        job.set_repro_only(false).unwrap();
        assert!(!job.repro_only());
        job.set_repro_only(true).unwrap();
        start(&job, &repo, "true", &[]).unwrap();
        assert!(job.exists() && job.repro_only());
        assert_eq!(wait_done(&job, 1), Poll::Done { exit_code: 0 });
        job.clear();
        assert!(!job.exists() && job.repro_only());
        discard_node(&repo, "T-1", "fix_gate").unwrap();
        assert!(!job.repro_only());
        // File-system failure must propagate rather than start an unmarked job.
        std::fs::create_dir(job.started.with_extension("repro-only")).unwrap();
        assert!(job.set_repro_only(true).is_err());
        assert!(job.set_repro_only(false).is_err());
        std::fs::remove_dir_all(repo).unwrap();
    }

    fn wait_done(job: &Job, timeout_minutes: u32) -> Poll {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            let p = poll(job, timeout_minutes);
            if p != Poll::Running || std::time::Instant::now() >= deadline {
                return p;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }

    #[test]
    fn setup_job_past_its_timeout_is_killed_with_124_and_a_setup_note() {
        let repo = sandbox("setup-timeout");
        for what in ["准备命令", "门禁"] {
            let job = Job::at(&repo, "T-1", "setup", what);
            start(&job, &repo, "sleep 30", &[]).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while std::fs::read_to_string(&job.pid)
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok())
                .is_none()
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert!(job.pid.exists());
            let pid: i32 = std::fs::read_to_string(&job.pid)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let result = if what == "门禁" {
                poll(&job, 0)
            } else {
                poll_as(&job, 0, what)
            };
            assert_eq!(result, Poll::Done { exit_code: 124 });
            assert!(tail(&job.log, 5).contains(&format!("{what}运行超过")));
            assert!(!crate::launcher::pid_alive(pid));
        }
    }

    #[cfg(unix)]
    #[test]
    fn setup_exit_published_between_marker_read_and_liveness_check_is_collected() {
        use std::io::Write;
        let repo = sandbox("setup-exit-race");
        let job = Job::at(&repo, "T-1", "setup", "worktree");
        std::fs::create_dir_all(job.pid.parent().unwrap()).unwrap();
        std::fs::write(&job.started, now_secs().to_string()).unwrap();
        assert!(
            Command::new("mkfifo")
                .arg(&job.pid)
                .status()
                .unwrap()
                .success()
        );
        let pid = job.pid.clone();
        let exit = job.exit.clone();
        let writer = std::thread::spawn(move || {
            // Opening the FIFO waits until poll has already read the absent exit marker.
            let mut file = std::fs::OpenOptions::new().write(true).open(pid).unwrap();
            write_marker(&exit, 0);
            writeln!(file, "{}", i32::MAX).unwrap();
        });
        let result = poll_as(&job, 1, "准备命令");
        writer.join().unwrap();
        assert_eq!(result, Poll::Done { exit_code: 0 });
    }

    #[test]
    fn setup_jobs_are_discarded_by_their_own_prefix_and_leave_gate_jobs_alone() {
        let repo = sandbox("setup-discard");
        let setup = Job::at(&repo, "T-1", "setup", "worktree");
        let gate = Job::at(&repo, "T-1", "gate", "abc");
        for job in [&setup, &gate] {
            start(job, &repo, "echo ok", &[]).unwrap();
            assert_eq!(wait_done(job, 5), Poll::Done { exit_code: 0 });
        }
        discard_node(&repo, "T-1", "setup").unwrap();
        assert!(!setup.started.exists() && !setup.log.exists() && !setup.exit.exists());
        assert!(gate.started.exists() && gate.log.exists() && gate.exit.exists());
    }

    #[test]
    fn setup_job_counts_as_running_from_its_started_file_until_its_exit_marker() {
        let repo = sandbox("setup-running");
        let job = Job::at(&repo, "T-1", "setup", "main");
        assert!(!running(&job));
        std::fs::create_dir_all(job.started.parent().unwrap()).unwrap();
        std::fs::write(&job.started, now_secs().to_string()).unwrap();
        assert!(running(&job));
        std::fs::write(&job.started, (now_secs() - 10).to_string()).unwrap();
        assert!(!running(&job));
        std::fs::write(&job.started, now_secs().to_string()).unwrap();
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        std::fs::write(&job.pid, child.id().to_string()).unwrap();
        assert!(running(&job));
        write_marker(&job.exit, 0);
        assert!(!running(&job));
        std::fs::remove_file(&job.exit).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!running(&job));
    }

    #[test]
    fn requeued_discard_waits_for_the_wrapper_pid_before_removing_files() {
        use std::os::unix::process::CommandExt;
        let repo = sandbox("discard-delayed-pid");
        let job = Job::at(&repo, "T-1", "gate", "abc");
        std::fs::create_dir_all(job.started.parent().unwrap()).unwrap();
        std::fs::write(&job.started, now_secs().to_string()).unwrap();
        // Hold the wrapper before its first pid write, reproducing the spawn window.
        let release = repo.join("release");
        let ready = repo.join("ready");
        let mut child = Command::new("/bin/sh")
            .arg("-c").arg("touch \"$READY\"; while [ ! -f \"$RELEASE\" ]; do sleep 0.01; done; echo $$ > \"$PID\"; while :; do echo stale >> \"$LOG\"; sleep 0.01; done")
            .env("READY", &ready).env("RELEASE", &release).env("PID", &job.pid).env("LOG", &job.log)
            .stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !ready.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(ready.exists());
        assert!(!job.pid.exists());
        let release_thread = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            std::fs::write(release, "go").unwrap();
        });
        discard_node(&repo, "T-1", "gate").unwrap();
        release_thread.join().unwrap();
        let exited = child.try_wait().unwrap().is_some();
        if !exited {
            kill_group(child.id() as i32);
        }
        child.wait().unwrap();
        assert!(
            exited,
            "old wrapper must be dead before paths can be reused"
        );
        assert!(!job.started.exists());
        assert!(!job.pid.exists());
        assert!(!job.log.exists());
        start(&job, &repo, "echo fresh", &[]).unwrap();
        assert_eq!(wait_done(&job, 5), Poll::Done { exit_code: 0 });
        assert_eq!(std::fs::read_to_string(&job.log).unwrap().trim(), "fresh");
        discard_node(&repo, "T-1", "gate").unwrap();
        discard_node(&repo, "missing", "gate").unwrap(); // A vanished wrapper has no pid or exit: the bounded wait must expire.
        std::fs::write(&job.started, "1").unwrap();
        let began = std::time::Instant::now();
        discard_node(&repo, "T-1", "gate").unwrap();
        assert!(began.elapsed() >= std::time::Duration::from_secs(3));
        assert!(!job.started.exists());

        std::fs::remove_dir_all(repo).unwrap();
    }

    #[test]
    fn a_passing_command_is_done_with_zero_and_its_output_is_kept() {
        let dir = sandbox("pass");
        let job = Job::at(&dir, "T-1", "gate", "abc");
        start(
            &job,
            &dir,
            "echo checked $AUTOME_HEAD",
            &[("AUTOME_HEAD", "abc")],
        )
        .unwrap();
        assert_eq!(wait_done(&job, 5), Poll::Done { exit_code: 0 });
        assert!(tail(&job.log, 10).contains("checked abc"));
        // The exit marker is published before the wrapper removes its pid.
        // Done confirms the command result, not that the wrapper has exited.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while job.pid.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            !job.pid.exists(),
            "the wrapper removes its pid on the way out"
        );
    }

    #[test]
    fn a_failing_command_reports_its_own_exit_code() {
        let dir = sandbox("fail");
        let job = Job::at(&dir, "T-1", "gate", "abc");
        start(&job, &dir, "echo broken >&2; exit 7", &[]).unwrap();
        assert_eq!(wait_done(&job, 5), Poll::Done { exit_code: 7 });
        assert!(
            tail(&job.log, 10).contains("broken"),
            "stderr is in the log too"
        );
    }

    #[test]
    fn a_gate_that_runs_too_long_is_killed_and_reported_as_timed_out() {
        let dir = sandbox("slow");
        let job = Job::at(&dir, "T-1", "gate", "abc");
        start(&job, &dir, "sleep 30", &[]).unwrap();
        // Wait for the pid, then pretend the job started an hour ago.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !job.pid.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        std::fs::write(&job.started, (now_secs() - 3600).to_string()).unwrap();
        assert_eq!(
            poll(&job, 1),
            Poll::Done {
                exit_code: TIMED_OUT
            }
        );
        assert!(tail(&job.log, 5).contains("已终止"));
    }

    #[test]
    fn a_job_with_no_process_and_no_marker_has_vanished() {
        let dir = sandbox("gone");
        let job = Job::at(&dir, "T-1", "gate", "abc");
        std::fs::create_dir_all(job.log.parent().unwrap()).unwrap();
        std::fs::write(&job.started, (now_secs() - 60).to_string()).unwrap();
        std::fs::write(&job.pid, "999999").unwrap();
        assert_eq!(poll(&job, 5), Poll::Vanished);
    }

    #[test]
    fn the_tail_is_bounded_and_cut_on_a_character_boundary() {
        let dir = sandbox("tail");
        let log = dir.join("x.log");
        std::fs::write(&log, "门".repeat(5000)).unwrap();
        let t = tail(&log, 10);
        assert!(t.len() <= 6000 + "…".len());
        assert!(t.starts_with('…'));
    }
}
