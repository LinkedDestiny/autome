//! Session records and the exit-marker protocol. Technical design §2, §7.
//!
//! A session is one CLI process run, launched into a visible iTerm2 tab rather
//! than as a child of `automed` (design §1, change 1). The core therefore
//! cannot `wait()` on it; it learns the session ended by watching for the
//! wrapper script's exit marker, with a pid liveness check and a log-mtime
//! heartbeat as the backstop for a user who closes the tab by hand (§17).
//!
//! This module owns the vocabulary of that protocol. The filesystem work
//! lives in `automed`.

use serde::{Deserialize, Serialize};

use crate::role::{Role, Runtime};

/// Which kind of work a session does. `Intake` and `Onboarding` are fixed
/// system prompts, not one of the five configurable roles, so they cannot be
/// represented by `Role` alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionKind {
    Intake,
    Onboarding,
    Role { role: Role },
}

impl SessionKind {
    pub fn role(self) -> Option<Role> {
        match self {
            SessionKind::Role { role } => Some(role),
            _ => None,
        }
    }

    /// Short label for the terminal tab title and the session list.
    pub fn label(self) -> String {
        match self {
            SessionKind::Intake => "任务整理".into(),
            SessionKind::Onboarding => "Onboarding".into(),
            SessionKind::Role { role } => match role {
                Role::Plan => "设计".into(),
                Role::Review => "评审".into(),
                Role::Adjudicate => "裁决".into(),
                Role::Impl => "实现".into(),
                Role::Audit => "审计".into(),
                Role::Retro => "复盘".into(),
            },
        }
    }
}

/// How a session finished, as read off disk before the design document is
/// parsed (design §7.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SessionLifecycle {
    /// Launched; no exit marker yet.
    Running,
    /// The wrapper wrote an exit marker.
    Exited { exit_code: i32 },
    /// `automed` killed it on the user's Stop.
    Killed,
    /// No exit marker, the pid is gone, and the log stopped growing long
    /// enough ago to rule out a slow session (§17).
    Vanished,
}

impl SessionLifecycle {
    pub fn is_running(&self) -> bool {
        matches!(self, SessionLifecycle::Running)
    }

    /// Whether the core should try to read the design document. A killed or
    /// vanished session may still have written something useful, but a
    /// non-zero exit is treated as a crash without parsing (§7.3).
    pub fn should_parse_document(&self) -> bool {
        matches!(self, SessionLifecycle::Exited { exit_code: 0 })
    }
}

/// The exit marker the wrapper script writes. Deliberately three plain lines
/// rather than JSON: it is produced by `sh`, and a partially-written marker
/// must be detectable rather than parse as valid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitMarker {
    pub exit_code: i32,
    pub ended_at: String,
}

impl ExitMarker {
    /// Serialises to the wrapper's format.
    pub fn render(&self) -> String {
        format!(
            "exit_code={}\nended_at={}\nmarker_end\n",
            self.exit_code, self.ended_at
        )
    }

    /// Parses a marker file. Returns `None` for a file that is absent, still
    /// being written, or corrupt — all of which mean "not finished yet", and
    /// none of which may be mistaken for `exit_code=0`.
    pub fn parse(contents: &str) -> Option<ExitMarker> {
        if !contents.contains("marker_end") {
            return None;
        }
        let mut exit_code = None;
        let mut ended_at = None;
        for line in contents.lines() {
            if let Some(v) = line.strip_prefix("exit_code=") {
                exit_code = v.trim().parse::<i32>().ok();
            } else if let Some(v) = line.strip_prefix("ended_at=") {
                ended_at = Some(v.trim().to_string());
            }
        }
        Some(ExitMarker {
            exit_code: exit_code?,
            ended_at: ended_at?,
        })
    }
}

/// One session, as recorded in the task's session history (requirement T-14).
/// `Eq` is deliberately absent: `SessionMetrics` carries a cost in dollars,
/// and a float has no total order worth pretending about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub task_id: String,
    pub kind: SessionKind,
    /// `None` for the two fixed-prompt kinds that always run Claude Code.
    pub runtime: Runtime,
    pub model: String,
    pub effort: Option<String>,
    pub skills: Vec<String>,
    /// Which loop round this session was; 0 for intake and onboarding.
    pub round: u32,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub lifecycle: SessionLifecycle,
    pub log_path: String,
    pub pid: Option<i32>,
    /// The protocol version this session ran under, in wire form
    /// (`protocol/v7@3f9a…`). `None` for sessions recorded before the protocol
    /// left the binary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_ref: Option<String>,
    /// Hash of the project's `.autome/rules/` at launch. Rules change between
    /// tasks, and a usage number that cannot say which rule set it was
    /// produced under cannot be compared with another.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules_hash: Option<String>,
    /// What the session cost, read out of the CLI's own stream when it is
    /// reaped. Empty when the stream could not be parsed — deliberately not
    /// zeroed, because zero is a claim.
    #[serde(default, flatten)]
    pub metrics: crate::metrics::SessionMetrics,
}

impl Session {
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }
}

/// The filenames the wrapper script and the core agree on, under
/// `.autome/output/sessions/<task-id>/`. Grouping by task keeps parallel
/// tasks from colliding (design §17).
pub struct SessionPaths;

impl SessionPaths {
    pub fn dir(task_id: &str) -> String {
        format!(".autome/output/sessions/{task_id}")
    }
    pub fn log(task_id: &str, session_id: &str) -> String {
        format!("{}/{session_id}.log", Self::dir(task_id))
    }
    pub fn pid(task_id: &str, session_id: &str) -> String {
        format!("{}/{session_id}.pid", Self::dir(task_id))
    }
    pub fn exit(task_id: &str, session_id: &str) -> String {
        format!("{}/{session_id}.exit", Self::dir(task_id))
    }
}

/// How long a log may go unmodified, with no pid and no exit marker, before
/// the core declares the session vanished (design §17).
pub const VANISHED_AFTER_SECS: u64 = 600;

/// Decides a session's lifecycle from what is observable on disk. Pure so the
/// backstop logic is testable without a filesystem or a clock.
///
/// The order matters: an exit marker is authoritative even if the process
/// happens to still be winding down, and a live pid always beats a stale log.
pub fn classify(
    marker: Option<&ExitMarker>,
    pid_alive: bool,
    log_idle_secs: u64,
) -> SessionLifecycle {
    match marker {
        Some(m) => SessionLifecycle::Exited {
            exit_code: m.exit_code,
        },
        None if pid_alive => SessionLifecycle::Running,
        None if log_idle_secs >= VANISHED_AFTER_SECS => SessionLifecycle::Vanished,
        // No marker, no pid, but the log moved recently: the wrapper is
        // probably between the CLI exiting and the marker being written.
        None => SessionLifecycle::Running,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_complete_marker_round_trips() {
        let m = ExitMarker {
            exit_code: 0,
            ended_at: "2026-09-15T14:02:11Z".into(),
        };
        assert_eq!(ExitMarker::parse(&m.render()), Some(m));
    }

    #[test]
    fn a_marker_without_its_terminator_is_not_parsed() {
        assert_eq!(ExitMarker::parse("exit_code=0\nended_at=t\n"), None);
    }

    #[test]
    fn a_truncated_marker_is_not_mistaken_for_success() {
        assert_eq!(ExitMarker::parse("exit_co"), None);
        assert_eq!(ExitMarker::parse("exit_code=\nmarker_end\n"), None);
        assert_eq!(ExitMarker::parse("ended_at=t\nmarker_end\n"), None);
    }

    #[test]
    fn a_non_zero_exit_parses_and_is_not_parsed_for_a_document() {
        let m = ExitMarker::parse("exit_code=137\nended_at=t\nmarker_end\n").unwrap();
        assert_eq!(m.exit_code, 137);
        let life = SessionLifecycle::Exited { exit_code: 137 };
        assert!(!life.should_parse_document());
    }

    #[test]
    fn only_a_clean_exit_triggers_document_parsing() {
        assert!(SessionLifecycle::Exited { exit_code: 0 }.should_parse_document());
        for life in [
            SessionLifecycle::Running,
            SessionLifecycle::Killed,
            SessionLifecycle::Vanished,
            SessionLifecycle::Exited { exit_code: 1 },
        ] {
            assert!(!life.should_parse_document(), "{life:?}");
        }
    }

    #[test]
    fn a_marker_wins_over_a_live_pid() {
        let m = ExitMarker {
            exit_code: 0,
            ended_at: "t".into(),
        };
        assert_eq!(
            classify(Some(&m), true, 0),
            SessionLifecycle::Exited { exit_code: 0 }
        );
    }

    #[test]
    fn a_live_pid_without_a_marker_is_still_running() {
        assert_eq!(classify(None, true, 99_999), SessionLifecycle::Running);
    }

    #[test]
    fn a_dead_pid_with_a_recently_written_log_is_given_time_to_write_its_marker() {
        assert_eq!(classify(None, false, 5), SessionLifecycle::Running);
    }

    #[test]
    fn a_dead_pid_and_a_long_idle_log_is_vanished() {
        assert_eq!(
            classify(None, false, VANISHED_AFTER_SECS),
            SessionLifecycle::Vanished
        );
        assert_eq!(
            classify(None, false, VANISHED_AFTER_SECS + 1),
            SessionLifecycle::Vanished
        );
    }

    #[test]
    fn session_paths_are_grouped_per_task() {
        assert_eq!(
            SessionPaths::log("T-15", "s1"),
            ".autome/output/sessions/T-15/s1.log"
        );
        assert_eq!(
            SessionPaths::exit("T-15", "s1"),
            ".autome/output/sessions/T-15/s1.exit"
        );
        assert_ne!(
            SessionPaths::dir("T-15"),
            SessionPaths::dir("T-16"),
            "parallel tasks must not share a session directory"
        );
    }

    #[test]
    fn session_kind_labels_every_variant() {
        assert_eq!(SessionKind::Intake.label(), "任务整理");
        for role in Role::ALL {
            let label = SessionKind::Role { role }.label();
            assert!(!label.is_empty());
        }
    }

    #[test]
    fn session_kind_exposes_its_role_only_for_role_sessions() {
        assert_eq!(SessionKind::Intake.role(), None);
        assert_eq!(
            SessionKind::Role { role: Role::Audit }.role(),
            Some(Role::Audit)
        );
    }

    #[test]
    fn session_round_trips_through_json() {
        let s = Session {
            id: "s1".into(),
            task_id: "T-1".into(),
            kind: SessionKind::Role { role: Role::Impl },
            runtime: Runtime::Claude,
            model: "claude-opus-5".into(),
            effort: Some("high".into()),
            skills: vec!["conventions".into()],
            round: 4,
            started_at: "t0".into(),
            ended_at: None,
            lifecycle: SessionLifecycle::Running,
            log_path: "p".into(),
            pid: Some(42),
            protocol_ref: None,
            rules_hash: None,
            metrics: Default::default(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Session>(&json).unwrap(), s);
    }
}
