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

/// The files one gate run owns.
#[derive(Debug, Clone)]
pub struct Job {
    pub log: PathBuf,
    pub pid: PathBuf,
    pub exit: PathBuf,
    pub started: PathBuf,
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
        }
    }

    /// Whether this job was ever started (and not cleared since).
    pub fn exists(&self) -> bool {
        self.started.exists()
    }

    /// Throws the run away: the commit it was for is not the one that
    /// matters any more.
    pub fn clear(&self) {
        for p in [&self.log, &self.pid, &self.exit, &self.started] {
            let _ = std::fs::remove_file(p);
        }
    }
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
    if let Some(dir) = job.log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    job.clear();
    std::fs::write(&job.started, now_secs().to_string())?;

    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(WRAPPER)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env("AUTOME_GATE_COMMAND", command)
        .env("AUTOME_GATE_LOG", &job.log)
        .env("AUTOME_GATE_PID", &job.pid)
        .env("AUTOME_GATE_EXIT", &job.exit);
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
    if let Some(marker) = std::fs::read_to_string(&job.exit)
        .ok()
        .and_then(|t| ExitMarker::parse(&t))
    {
        return Poll::Done {
            exit_code: marker.exit_code,
        };
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
                    format!("\n=== Autome：门禁运行超过 {timeout_minutes} 分钟，已终止 ===\n");
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
        _ => Poll::Vanished,
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

/// The last `lines` lines of a log, for the prompt and the panel. Bounded in
/// bytes too: one line of minified output must not become the whole prompt.
pub fn tail(log: &Path, lines: usize) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
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

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("automed-gate-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
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
