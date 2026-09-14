//! Narrow, mechanically-defined slice of the Claude adapter (plan §4.2,
//! D8): a newline-delimited stream-json transport over `claude -p
//! --input-format stream-json --output-format stream-json`'s stdio, plus a
//! single-turn `probe_turn` built on it.
//!
//! Wire format confirmed empirically against the real `claude` binary
//! (`claude-cli 2.1.261`, `--bare`, no `ANTHROPIC_API_KEY` set): one JSON
//! object per line, but unlike Codex's `app-server` this is *not*
//! JSON-RPC — lines carry no `id`/`jsonrpc` envelope, just a `type` tag.
//! Confirmed sequence for one user turn: an unsolicited
//! `type:"system",subtype:"init"` line first (not a response to any
//! request — carries `session_id`, `cwd`, `tools`, `model`,
//! `apiKeySource`, ...), then one or more `type:"assistant"` message
//! lines, then a terminal `type:"result"` line (`is_error`, `subtype`,
//! `result` text, `terminal_reason`, `total_cost_usd`, ...). Also
//! confirmed empirically: `--output-format stream-json` requires
//! `--verbose` when combined with `--print`/`-p` — omitting it is a CLI
//! validation error ("Error: When using --print, --output-format=stream-
//! json requires --verbose"), not a runtime one. Also confirmed: after
//! writing one `type:"user"` stream-json line and closing stdin, the
//! process runs the turn to completion and exits **on its own** (no
//! forced kill needed on the normal path) — `--bare` with no credentials
//! configured produced exit code 1.
//!
//! The one failure mode probed so far: with `--bare` and no
//! `ANTHROPIC_API_KEY`/`apiKeyHelper` configured, the init line's
//! `apiKeySource` reads `"none"`, the assistant message carries
//! `"error":"authentication_failed"` and `"is_api_error_message":true`
//! with synthetic content (`"Not logged in · Please run /login"`,
//! `"model":"<synthetic>"`, all-zero token usage), and the terminal result
//! line carries `"is_error":true` and `"terminal_reason":"api_error"`.
//! This module models exactly that shape. It deliberately does not model:
//! a genuine authenticated/successful turn (no credentials available in
//! this environment to observe one — same honestly-scoped gap as Codex's
//! `turn/start` lifecycle in `codex_transport.rs`), multi-turn/resumed
//! sessions, tool-use content blocks, interrupt, or the dedicated-API-key
//! `apiKeyHelper` non-leakage proof the plan requires before this adapter
//! can ship (plan line 123 — a hard blocker this module does not attempt
//! to satisfy).
//!
//! D8 isolation this module is responsible for: `ClaudeTransport::spawn`
//! takes `claude_config_dir` as an `fs_guard::OwnedDirGuard`, exactly like
//! Codex's `CODEX_HOME` in `codex_transport.rs` — structurally impossible
//! to spawn `claude` against a directory that hasn't passed the
//! owner-only check. This is **not** a complete identity-isolation
//! guarantee, though: the plan explicitly warns `CLAUDE_CONFIG_DIR` does
//! not isolate the shared macOS Keychain OAuth identity, and `--bare`
//! merely *refuses to read* that identity (confirmed here by
//! `apiKeySource:"none"`), it doesn't relocate it. Spawns with a cleared
//! environment plus exactly `CLAUDE_CONFIG_DIR` and `PATH`.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

use crate::fs_guard::OwnedDirGuard;

#[derive(Debug, Error)]
pub enum ClaudeTransportError {
    #[error("failed to spawn {binary:?}: {source}")]
    Spawn {
        binary: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write the user turn to child stdin: {0}")]
    WriteRequest(#[source] std::io::Error),
    #[error("failed to read a line from child stdout: {0}")]
    ReadResponse(#[source] std::io::Error),
    #[error("child closed stdout before a result line arrived")]
    ClosedStdout,
    #[error("stdout line was not valid JSON: {0}")]
    MalformedLine(#[source] serde_json::Error),
    #[error("result line was valid JSON but missing an expected field: {line}")]
    UnexpectedEnvelope { line: String },
    #[error("turn timed out after {timeout_ms}ms and the child was killed")]
    Timeout { timeout_ms: u64 },
}

/// One decoded stdout line, classified only by its `type`/`subtype` tags.
/// Anything this module has no documented rule for yet (tool-use content
/// blocks, a `"user"` echo line, ...) is surfaced as `Other` rather than
/// silently dropped or guessed at — the same non-guessing discipline
/// `codex_transport.rs` applies to `CodexFrame::Unmatched`.
#[derive(Debug, Clone, PartialEq)]
enum ClaudeFrame {
    Init(Value),
    Assistant(Value),
    Result(Value),
    Other(Value),
}

fn classify(value: &Value) -> ClaudeFrame {
    match value.get("type").and_then(Value::as_str) {
        Some("system") if value.get("subtype").and_then(Value::as_str) == Some("init") => {
            ClaudeFrame::Init(value.clone())
        }
        Some("assistant") => ClaudeFrame::Assistant(value.clone()),
        Some("result") => ClaudeFrame::Result(value.clone()),
        _ => ClaudeFrame::Other(value.clone()),
    }
}

/// The terminal `type:"result"` line. `is_error`/`terminal_reason` are the
/// confirmed way to detect the one failure mode this module has observed
/// (see module doc) — a genuine successful turn's shape has not been
/// observed against a real credential in this environment, so no
/// success-specific fields are parsed beyond what's common to both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaudeResultSummary {
    pub is_error: bool,
    pub subtype: Option<String>,
    pub result_text: Option<String>,
    pub terminal_reason: Option<String>,
}

fn parse_result_summary(value: &Value) -> Result<ClaudeResultSummary, ClaudeTransportError> {
    let is_error = value.get("is_error").and_then(Value::as_bool).ok_or_else(|| {
        ClaudeTransportError::UnexpectedEnvelope { line: value.to_string() }
    })?;
    Ok(ClaudeResultSummary {
        is_error,
        subtype: value.get("subtype").and_then(Value::as_str).map(str::to_string),
        result_text: value.get("result").and_then(Value::as_str).map(str::to_string),
        terminal_reason: value.get("terminal_reason").and_then(Value::as_str).map(str::to_string),
    })
}

/// Outcome of one probed turn: the init line's session id/model/
/// `apiKeySource`, every `assistant`/`Other` line seen along the way (kept
/// verbatim, not parsed further — see module doc), and the terminal result
/// summary.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeTurnOutcome {
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub api_key_source: Option<String>,
    pub assistant_lines: Vec<Value>,
    pub other_lines: Vec<Value>,
    pub result: ClaudeResultSummary,
}

/// A running `claude -p --input-format stream-json --output-format
/// stream-json` child process plus its line-oriented stdio halves. Models
/// exactly one turn: `send_user_text_and_close_stdin` then
/// `read_until_result`, matching what's been confirmed empirically (see
/// module doc) rather than a general-purpose multi-turn session.
pub struct ClaudeTransport {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl ClaudeTransport {
    /// Spawns `claude_binary` with the plan's fixed §4.2.2 argv
    /// (`-p --input-format stream-json --output-format stream-json
    /// --verbose --bare --session-id <session_id> --permission-mode
    /// dontAsk --permission-prompts none`) under `claude_config_dir`, with
    /// a cleared environment carrying only `PATH` and `CLAUDE_CONFIG_DIR`.
    /// `claude_config_dir` is an `OwnedDirGuard`, not a bare path, so
    /// reaching this call already proves the owner-only check (D8) passed.
    pub fn spawn(
        claude_binary: &Path,
        claude_config_dir: &OwnedDirGuard,
        session_id: &str,
    ) -> Result<Self, ClaudeTransportError> {
        let mut command = tokio::process::Command::new(claude_binary);
        command
            .arg("-p")
            .arg("--input-format")
            .arg("stream-json")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .arg("--bare")
            .arg("--session-id")
            .arg(session_id)
            .arg("--permission-mode")
            .arg("dontAsk")
            .arg("--permission-prompts")
            .arg("none")
            .env_clear()
            .env("CLAUDE_CONFIG_DIR", &claude_config_dir.canonical_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Ok(path) = std::env::var("PATH") {
            command.env("PATH", path);
        }

        let mut child = command.spawn().map_err(|source| ClaudeTransportError::Spawn {
            binary: claude_binary.to_path_buf(),
            source,
        })?;
        let stdin = child.stdin.take().expect("stdin was piped at spawn");
        let stdout = BufReader::new(child.stdout.take().expect("stdout was piped at spawn"));

        Ok(Self { child, stdin: Some(stdin), stdout })
    }

    /// Writes one `type:"user"` stream-json line carrying `text`, then
    /// drops the `ChildStdin` half to close it. Confirmed empirically this
    /// is what signals "no more turns" and lets the process exit on its
    /// own once the current turn finishes — callable at most once per
    /// transport (panics otherwise, mirroring the single-turn scope this
    /// module documents).
    pub async fn send_user_text_and_close_stdin(
        &mut self,
        text: &str,
    ) -> Result<(), ClaudeTransportError> {
        let mut stdin = self.stdin.take().expect("send called at most once per transport");
        let envelope = serde_json::json!({
            "type": "user",
            "message": { "role": "user", "content": [{"type": "text", "text": text}] }
        });
        let mut line = serde_json::to_vec(&envelope).expect("a json! object always serializes");
        line.push(b'\n');
        stdin.write_all(&line).await.map_err(ClaudeTransportError::WriteRequest)?;
        stdin.flush().await.map_err(ClaudeTransportError::WriteRequest)?;
        drop(stdin);
        Ok(())
    }

    /// Reads lines until the terminal `result` line or EOF. Does not kill
    /// the child on error or on success — the caller (`probe_turn`) owns
    /// that decision, mirroring `CodexTransport`'s convention.
    pub async fn read_until_result(&mut self) -> Result<ClaudeTurnOutcome, ClaudeTransportError> {
        let mut session_id = None;
        let mut model = None;
        let mut api_key_source = None;
        let mut assistant_lines = Vec::new();
        let mut other_lines = Vec::new();

        loop {
            let mut raw = String::new();
            let bytes_read = self
                .stdout
                .read_line(&mut raw)
                .await
                .map_err(ClaudeTransportError::ReadResponse)?;
            if bytes_read == 0 {
                return Err(ClaudeTransportError::ClosedStdout);
            }
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }

            let value: Value =
                serde_json::from_str(trimmed).map_err(ClaudeTransportError::MalformedLine)?;

            match classify(&value) {
                ClaudeFrame::Init(v) => {
                    session_id = v.get("session_id").and_then(Value::as_str).map(str::to_string);
                    model = v.get("model").and_then(Value::as_str).map(str::to_string);
                    api_key_source =
                        v.get("apiKeySource").and_then(Value::as_str).map(str::to_string);
                }
                ClaudeFrame::Assistant(v) => assistant_lines.push(v),
                ClaudeFrame::Result(v) => {
                    let result = parse_result_summary(&v)?;
                    return Ok(ClaudeTurnOutcome {
                        session_id,
                        model,
                        api_key_source,
                        assistant_lines,
                        other_lines,
                        result,
                    });
                }
                ClaudeFrame::Other(v) => other_lines.push(v),
            }
        }
    }
}

async fn kill_and_reap(transport: &mut ClaudeTransport) {
    let _ = transport.child.kill().await;
    let _ = transport.child.wait().await;
}

/// Spawns, sends one user turn, and reads to completion, then kills the
/// probe child only if it hasn't exited on its own within a short grace
/// period — this is a stateless capability probe, not a long-lived
/// session, and mirrors `codex_transport.rs`'s "probe, don't hold open"
/// shape while still respecting the no-leaked-process discipline
/// `harness_probe.rs` established (every non-success path here explicitly
/// kills and reaps; the confirmed-clean-exit path still verifies the exit
/// rather than assuming it).
pub async fn probe_turn(
    claude_binary: &Path,
    claude_config_dir: &OwnedDirGuard,
    text: &str,
    timeout: Duration,
) -> Result<ClaudeTurnOutcome, ClaudeTransportError> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let mut transport = ClaudeTransport::spawn(claude_binary, claude_config_dir, &session_id)?;

    if let Err(err) = transport.send_user_text_and_close_stdin(text).await {
        kill_and_reap(&mut transport).await;
        return Err(err);
    }

    match tokio::time::timeout(timeout, transport.read_until_result()).await {
        Ok(Ok(outcome)) => {
            if tokio::time::timeout(Duration::from_secs(5), transport.child.wait())
                .await
                .is_err()
            {
                kill_and_reap(&mut transport).await;
            }
            Ok(outcome)
        }
        Ok(Err(err)) => {
            kill_and_reap(&mut transport).await;
            Err(err)
        }
        Err(_) => {
            kill_and_reap(&mut transport).await;
            Err(ClaudeTransportError::Timeout { timeout_ms: timeout.as_millis() as u64 })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn real_claude_config_dir() -> OwnedDirGuard {
        let root = std::env::temp_dir().join(format!(
            "automed-claude-transport-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut perms = std::fs::metadata(&root).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o700);
        std::fs::set_permissions(&root, perms).unwrap();
        crate::fs_guard::create_owned_dir(&root, "claude-config-dir").unwrap()
    }

    /// Real-binary integration test: a genuine `claude -p --input-format
    /// stream-json --output-format stream-json --bare` (`claude-cli
    /// 2.1.261`, no `ANTHROPIC_API_KEY` set) speaks the exact
    /// newline-delimited event-stream framing this module assumes, and the
    /// one failure mode this module documents — no credentials under
    /// `--bare` — round-trips exactly as the module doc describes:
    /// `apiKeySource:"none"`, an `authentication_failed` assistant line,
    /// and a terminal `is_error:true`/`terminal_reason:"api_error"` result.
    #[tokio::test]
    async fn probe_turn_round_trips_the_unauthenticated_failure_mode_against_the_real_claude_binary(
    ) {
        let config_dir = real_claude_config_dir();
        let outcome = probe_turn(
            Path::new("/Users/dannie/.local/bin/claude"),
            &config_dir,
            "say hi",
            Duration::from_secs(20),
        )
        .await
        .unwrap();

        assert_eq!(outcome.api_key_source.as_deref(), Some("none"));
        assert!(outcome.session_id.is_some_and(|id| !id.is_empty()));
        assert!(
            outcome.assistant_lines.iter().any(|line| {
                line.get("error").and_then(Value::as_str) == Some("authentication_failed")
            }),
            "expected an assistant line flagging authentication_failed, got {:?}",
            outcome.assistant_lines
        );
        assert!(outcome.result.is_error);
        assert_eq!(outcome.result.terminal_reason.as_deref(), Some("api_error"));

        std::fs::remove_dir_all(config_dir.canonical_path.parent().unwrap()).ok();
    }

    /// A stand-in binary that never writes a line and never exits must be
    /// killed and reaped, not left running, and `probe_turn` must return
    /// promptly rather than waiting out the fixture's own much longer
    /// sleep.
    #[tokio::test]
    async fn a_hung_claude_binary_times_out_and_is_killed_not_leaked() {
        let config_dir = real_claude_config_dir();
        let started = std::time::Instant::now();
        let err = probe_turn(
            &fixture("fake_harness_hangs.sh"),
            &config_dir,
            "say hi",
            Duration::from_millis(200),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, ClaudeTransportError::Timeout { .. }));
        assert!(started.elapsed() < Duration::from_secs(5));
        std::fs::remove_dir_all(config_dir.canonical_path.parent().unwrap()).ok();
    }

    /// A stand-in binary whose first line is not JSON must surface a typed
    /// parse error, never panic or hang waiting for a well-formed line
    /// that will never arrive.
    #[tokio::test]
    async fn a_garbage_first_line_yields_a_typed_parse_error() {
        let config_dir = real_claude_config_dir();
        let err = probe_turn(
            &fixture("fake_claude_garbage.sh"),
            &config_dir,
            "say hi",
            Duration::from_secs(2),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, ClaudeTransportError::MalformedLine(_)));
        std::fs::remove_dir_all(config_dir.canonical_path.parent().unwrap()).ok();
    }

    /// A stand-in binary that exits immediately without writing anything
    /// must surface `ClosedStdout`, not hang or panic on EOF.
    #[tokio::test]
    async fn a_closed_stdout_before_any_result_is_reported_not_hung() {
        let config_dir = real_claude_config_dir();
        let err = probe_turn(
            &fixture("fake_claude_closes_immediately.sh"),
            &config_dir,
            "say hi",
            Duration::from_secs(2),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, ClaudeTransportError::ClosedStdout));
        std::fs::remove_dir_all(config_dir.canonical_path.parent().unwrap()).ok();
    }

    /// D8 rejection: `ClaudeTransport::spawn`/`probe_turn` take an
    /// `OwnedDirGuard`, not a bare path, and that guard cannot be
    /// constructed for a group/world-writable directory —
    /// `verify_owned_dir` rejects it first. There is no bypass path from
    /// an unsafe `CLAUDE_CONFIG_DIR` into a spawned `claude` process,
    /// mirroring `codex_transport.rs`'s equivalent proof for `CODEX_HOME`.
    #[test]
    fn an_unsafe_claude_config_dir_never_produces_a_guard_spawn_could_accept() {
        let root = std::env::temp_dir().join(format!(
            "automed-claude-transport-test-unsafe-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut perms = std::fs::metadata(&root).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o700);
        std::fs::set_permissions(&root, perms).unwrap();

        let unsafe_dir = root.join("world-writable-claude-config-dir");
        std::fs::create_dir(&unsafe_dir).unwrap();
        let mut perms = std::fs::metadata(&unsafe_dir).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o777);
        std::fs::set_permissions(&unsafe_dir, perms).unwrap();

        let err = crate::fs_guard::verify_owned_dir(&unsafe_dir, &root).unwrap_err();
        assert!(matches!(
            err,
            crate::fs_guard::FsGuardError::GroupOrWorldWritable { .. }
        ));
        std::fs::remove_dir_all(&root).ok();
    }

    /// `classify` treats `system`/`init` and `result` lines specially and
    /// leaves everything else as `Other` rather than guessing at unmodeled
    /// shapes.
    #[test]
    fn classify_distinguishes_init_assistant_result_and_other_lines() {
        let init = serde_json::json!({"type": "system", "subtype": "init", "session_id": "x"});
        let assistant = serde_json::json!({"type": "assistant", "message": {}});
        let result = serde_json::json!({"type": "result", "is_error": false});
        let other = serde_json::json!({"type": "user", "message": {}});

        assert!(matches!(classify(&init), ClaudeFrame::Init(_)));
        assert!(matches!(classify(&assistant), ClaudeFrame::Assistant(_)));
        assert!(matches!(classify(&result), ClaudeFrame::Result(_)));
        assert!(matches!(classify(&other), ClaudeFrame::Other(_)));
    }
}
