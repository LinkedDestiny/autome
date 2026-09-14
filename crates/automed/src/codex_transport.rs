//! Narrow, mechanically-defined slice of the Codex adapter (plan §4.2, D8):
//! a newline-delimited JSON-RPC 2.0 transport over `codex app-server`'s
//! stdio, plus `probe_initialize`/`probe_model_list`/`probe_account_read`/
//! `probe_thread_start` built on it.
//!
//! Wire format confirmed empirically against the real `codex` binary
//! (`codex-cli 0.153.4`): one JSON object per line on stdin/stdout, no
//! `Content-Length` framing — unlike an LSP-style transport, a line *is* a
//! message. Also confirmed empirically: `model/list` and `thread/start`
//! both succeed without any account configured, while
//! `account/rateLimits/read` and `account/usage/read` return a JSON-RPC
//! error (`-32600`) until logged in, and `account/read` itself answers
//! `{"account": null, "requiresOpenaiAuth": true}` rather than erroring —
//! three different unauthenticated-state shapes on three related
//! endpoints, none of them guessed. This module models the four probes
//! above plus `probe_turn_to_completion`; it deliberately does not
//! implement `account/rateLimits/read`, `account/usage/read`, dynamic
//! tools, sandboxed command execution, `turn/interrupt`, `thread/resume`,
//! or `ServerRequest` approval-deny handling (§4.2.1). None of those
//! remaining unimplemented pieces have a confirmed shape, and guessing
//! here would repeat the mistake `harness_probe.rs`'s module doc already
//! warns against.
//!
//! `turn/start`'s full lifecycle, confirmed empirically (`codex-cli
//! 0.153.4`, no account configured): its own JSON-RPC response resolves
//! immediately with `{"turn": {"status": "inProgress", ...}}` — it does
//! *not* fail synchronously the way `account/*` does. Real completion
//! arrives later as an unsolicited, `id`-less notification line,
//! `{"method": "turn/completed", "params": {"threadId": ..., "turn": {...,
//! "status": "failed", "error": {"message": "unexpected status 401
//! Unauthorized: ...", ...}, "startedAt": ..., "completedAt": ...,
//! "durationMs": ...}}}`. Between `turn/start`'s response and
//! `turn/completed`, an unauthenticated turn produces substantial
//! interleaved notification traffic — `thread/started`,
//! `thread/status/changed`, `turn/started`, `item/started`/`item/
//! completed` for the echoed user message, then repeated `method:"error"`
//! reconnect-attempt notifications (`"Reconnecting... N/5"`, `willRetry:
//! true`, `codexErrorInfo.responseStreamDisconnected.httpStatusCode:401`)
//! as the client retries over WebSocket, a `method:"warning"` when it
//! falls back to HTTPS transport, then a second round of `N/5` reconnect
//! attempts over HTTPS before giving up — roughly 30-40 seconds
//! end-to-end for the one failure mode observed. `probe_turn_to_completion`
//! drives exactly this: it keeps every notification line it sees along the
//! way (never silently dropped, matching `request()`'s own `skipped`
//! discipline) but only surfaces the terminal `turn/completed` line's
//! `status`/`error.message` — the same intentionally-narrow scope this
//! module applies everywhere else. A genuine *successful* turn's
//! `turn/completed` shape has not been observed (no account configured in
//! this environment) and is not guessed at here.
//!
//! D8 isolation this module is responsible for: `spawn` takes `codex_home`
//! as an `fs_guard::OwnedDirGuard`, not a bare `&Path` — that type only
//! exists once `fs_guard::create_owned_dir`/`verify_owned_dir` has already
//! passed the full owner-only check, so it is structurally impossible to
//! spawn `codex` against a directory this module hasn't confirmed is
//! owner-only. This module spawns with a cleared environment plus exactly
//! `CODEX_HOME` and `PATH` — never inheriting the parent's ambient
//! environment wholesale.

use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

use crate::fs_guard::OwnedDirGuard;

#[derive(Debug, Error)]
pub enum CodexTransportError {
    #[error("failed to spawn {binary:?}: {source}")]
    Spawn {
        binary: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write request to child stdin: {0}")]
    WriteRequest(#[source] std::io::Error),
    #[error("failed to read response line from child stdout: {0}")]
    ReadResponse(#[source] std::io::Error),
    #[error("child closed stdout before a matching response arrived")]
    ClosedStdout,
    #[error("response line was not valid JSON: {0}")]
    MalformedLine(#[source] serde_json::Error),
    #[error("response envelope was valid JSON but not a JSON-RPC object with the expected id")]
    UnexpectedEnvelope { line: String },
    #[error("server returned a JSON-RPC error: code={code} message={message}")]
    RpcError { code: i64, message: String },
    #[error("request timed out after {timeout_ms}ms and the child was killed")]
    Timeout { timeout_ms: u64 },
}

/// One line of newline-delimited JSON-RPC traffic from the child's stdout,
/// classified only as far as distinguishing "this is the response to the
/// request I sent" from everything else. Server-originated requests and
/// notifications are real `codex app-server` traffic (approval prompts,
/// progress events, ...) but this module has no documented rule yet for
/// handling them — they are surfaced verbatim as `Unmatched` rather than
/// silently dropped or guessed at.
#[derive(Debug, Clone, PartialEq)]
pub enum CodexFrame {
    Response(Value),
    Unmatched(Value),
}

/// A running `codex app-server` child process plus its line-oriented stdio
/// halves. Holds nothing beyond what's needed for one request/response
/// round trip at a time; concurrent in-flight requests and background
/// draining of unmatched frames are out of scope for this slice.
pub struct CodexTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: AtomicU64,
}

impl CodexTransport {
    /// Spawns `codex_binary app-server` under `codex_home`, with a cleared
    /// environment carrying only `PATH` and `CODEX_HOME`. `codex_home` is an
    /// `OwnedDirGuard`, not a bare path, so reaching this call already
    /// proves the owner-only check (D8) passed.
    pub fn spawn(
        codex_binary: &Path,
        codex_home: &OwnedDirGuard,
    ) -> Result<Self, CodexTransportError> {
        let mut command = tokio::process::Command::new(codex_binary);
        command
            .arg("app-server")
            .env_clear()
            .env("CODEX_HOME", &codex_home.canonical_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Ok(path) = std::env::var("PATH") {
            command.env("PATH", path);
        }

        let mut child = command.spawn().map_err(|source| CodexTransportError::Spawn {
            binary: codex_binary.to_path_buf(),
            source,
        })?;
        let stdin = child.stdin.take().expect("stdin was piped at spawn");
        let stdout = BufReader::new(child.stdout.take().expect("stdout was piped at spawn"));

        Ok(Self {
            child,
            stdin,
            stdout,
            next_id: AtomicU64::new(1),
        })
    }

    /// Sends `method`/`params` as a JSON-RPC request and waits up to
    /// `timeout` for the line carrying the matching `id`. Any line read
    /// before that (a server-originated request/notification, or a
    /// response to some other id) is returned to the caller as
    /// `CodexFrame::Unmatched` interleaved in `skipped`, never dropped
    /// silently. On timeout the child is killed and reaped — the same
    /// no-leaked-process discipline `harness_probe.rs` applies.
    pub async fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<(Value, Vec<Value>), CodexTransportError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let envelope = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let mut line = serde_json::to_vec(&envelope).expect("a json! object always serializes");
        line.push(b'\n');

        match tokio::time::timeout(timeout, self.write_and_await(id, &line)).await {
            Ok(result) => result,
            Err(_) => {
                let _ = self.child.kill().await;
                let _ = self.child.wait().await;
                Err(CodexTransportError::Timeout {
                    timeout_ms: timeout.as_millis() as u64,
                })
            }
        }
    }

    async fn write_and_await(
        &mut self,
        id: u64,
        line: &[u8],
    ) -> Result<(Value, Vec<Value>), CodexTransportError> {
        self.stdin
            .write_all(line)
            .await
            .map_err(CodexTransportError::WriteRequest)?;
        self.stdin
            .flush()
            .await
            .map_err(CodexTransportError::WriteRequest)?;

        let mut skipped = Vec::new();
        loop {
            let mut raw = String::new();
            let bytes_read = self
                .stdout
                .read_line(&mut raw)
                .await
                .map_err(CodexTransportError::ReadResponse)?;
            if bytes_read == 0 {
                return Err(CodexTransportError::ClosedStdout);
            }
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }

            let value: Value =
                serde_json::from_str(trimmed).map_err(CodexTransportError::MalformedLine)?;

            match classify(&value, id) {
                CodexFrame::Response(v) => return finish_response(v, skipped),
                CodexFrame::Unmatched(v) => skipped.push(v),
            }
        }
    }

    /// Reads further stdout lines — no new write — until an unsolicited
    /// notification whose top-level `method` equals `notification_method`
    /// arrives, or `timeout` elapses. Every other line read along the way
    /// (any other notification, or a response to some request) is kept in
    /// the returned `Vec`, mirroring `request()`'s own never-drop-silently
    /// discipline for `skipped`. On timeout the child is killed and
    /// reaped, same as `request()`.
    async fn read_until_notification(
        &mut self,
        notification_method: &str,
        timeout: Duration,
    ) -> Result<(Value, Vec<Value>), CodexTransportError> {
        match tokio::time::timeout(timeout, self.read_until_notification_inner(notification_method))
            .await
        {
            Ok(result) => result,
            Err(_) => {
                let _ = self.child.kill().await;
                let _ = self.child.wait().await;
                Err(CodexTransportError::Timeout {
                    timeout_ms: timeout.as_millis() as u64,
                })
            }
        }
    }

    async fn read_until_notification_inner(
        &mut self,
        notification_method: &str,
    ) -> Result<(Value, Vec<Value>), CodexTransportError> {
        let mut skipped = Vec::new();
        loop {
            let mut raw = String::new();
            let bytes_read = self
                .stdout
                .read_line(&mut raw)
                .await
                .map_err(CodexTransportError::ReadResponse)?;
            if bytes_read == 0 {
                return Err(CodexTransportError::ClosedStdout);
            }
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }

            let value: Value =
                serde_json::from_str(trimmed).map_err(CodexTransportError::MalformedLine)?;
            if value.get("method").and_then(Value::as_str) == Some(notification_method) {
                return Ok((value, skipped));
            }
            skipped.push(value);
        }
    }
}

/// Classifies one decoded JSON-RPC line against the `id` we're waiting for.
/// A value that isn't even an object with an `id` field is still a valid
/// frame (e.g. a notification) — it's simply `Unmatched`, not an error;
/// only a genuinely unparsable *line* is an error (see `MalformedLine`).
fn classify(value: &Value, expected_id: u64) -> CodexFrame {
    let matches_id = value
        .as_object()
        .and_then(|obj| obj.get("id"))
        .and_then(Value::as_u64)
        .map(|id| id == expected_id)
        .unwrap_or(false);
    if matches_id {
        CodexFrame::Response(value.clone())
    } else {
        CodexFrame::Unmatched(value.clone())
    }
}

fn finish_response(
    value: Value,
    skipped: Vec<Value>,
) -> Result<(Value, Vec<Value>), CodexTransportError> {
    let obj = value.as_object().ok_or_else(|| CodexTransportError::UnexpectedEnvelope {
        line: value.to_string(),
    })?;
    if let Some(error) = obj.get("error") {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("<no message>")
            .to_string();
        return Err(CodexTransportError::RpcError { code, message });
    }
    let result = obj
        .get("result")
        .cloned()
        .ok_or_else(|| CodexTransportError::UnexpectedEnvelope {
            line: value.to_string(),
        })?;
    Ok((result, skipped))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexInitializeResult {
    pub user_agent: Option<String>,
    pub codex_home: Option<String>,
    pub platform_family: Option<String>,
    pub platform_os: Option<String>,
}

/// Spawns `codex_binary app-server` under `codex_home` and performs the
/// `initialize` handshake, leaving the transport open for one more call.
/// Empirically, `codex app-server` (`codex-cli 0.153.4`) also emits an
/// unsolicited `remoteControl/status/changed` notification somewhere around
/// this handshake — real traffic the caller's next `request()` will see as
/// `Unmatched` and correctly skip, not an error case.
async fn spawn_initialized(
    codex_binary: &Path,
    codex_home: &OwnedDirGuard,
    timeout: Duration,
) -> Result<(CodexTransport, CodexInitializeResult), CodexTransportError> {
    let mut transport = CodexTransport::spawn(codex_binary, codex_home)?;
    let params = serde_json::json!({
        "clientInfo": {
            "name": "autome",
            "title": "Autome",
            "version": env!("CARGO_PKG_VERSION"),
        }
    });
    match transport.request("initialize", params, timeout).await {
        Ok((result, _skipped)) => Ok((transport, parse_initialize_result(result))),
        Err(err) => {
            // `request()` already kills+reaps on its own Timeout path; the
            // other error variants (malformed line, closed stdout, rpc
            // error, ...) return with the child still alive, so this call
            // still owns the kill here too — the same discipline
            // `probe_initialize` used to apply unconditionally before this
            // helper existed.
            kill_and_reap(&mut transport).await;
            Err(err)
        }
    }
}

async fn kill_and_reap(transport: &mut CodexTransport) {
    let _ = transport.child.kill().await;
    let _ = transport.child.wait().await;
}

/// Spawns `codex_binary app-server` under `codex_home` and performs a single
/// `initialize` round trip, then kills the probe child — this is a
/// stateless capability probe, not a long-lived session. Mirrors
/// `harness_probe::probe_harness_binary`'s "probe, don't hold open" shape.
pub async fn probe_initialize(
    codex_binary: &Path,
    codex_home: &OwnedDirGuard,
    timeout: Duration,
) -> Result<CodexInitializeResult, CodexTransportError> {
    match spawn_initialized(codex_binary, codex_home, timeout).await {
        Ok((mut transport, initialize_result)) => {
            kill_and_reap(&mut transport).await;
            Ok(initialize_result)
        }
        Err(err) => Err(err),
    }
}

fn parse_initialize_result(result: Value) -> CodexInitializeResult {
    CodexInitializeResult {
        user_agent: result
            .get("userAgent")
            .and_then(Value::as_str)
            .map(str::to_string),
        codex_home: result
            .get("codexHome")
            .and_then(Value::as_str)
            .map(str::to_string),
        platform_family: result
            .get("platformFamily")
            .and_then(Value::as_str)
            .map(str::to_string),
        platform_os: result
            .get("platformOs")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

/// One entry from `model/list`'s `data` array. Only the fields this probe
/// actually needs are modeled — the real response carries many more
/// (`supportedReasoningEfforts`, `serviceTiers`, `inputModalities`, ...)
/// that have no consumer yet and are deliberately left unparsed rather than
/// guessed into a struct shape nothing uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexModelSummary {
    pub id: String,
    pub display_name: Option<String>,
    pub is_default: bool,
}

/// Spawns, initializes, and calls `model/list`, then kills the probe child.
/// Empirically (`codex-cli 0.153.4`, no auth configured) this succeeds
/// without any account being logged in — model listing is not
/// authentication-gated the way `account/rateLimits/read` and
/// `account/usage/read` are.
pub async fn probe_model_list(
    codex_binary: &Path,
    codex_home: &OwnedDirGuard,
    timeout: Duration,
) -> Result<Vec<CodexModelSummary>, CodexTransportError> {
    let (mut transport, _initialize_result) =
        spawn_initialized(codex_binary, codex_home, timeout).await?;

    let request_result = transport.request("model/list", serde_json::json!({}), timeout).await;
    kill_and_reap(&mut transport).await;
    let (result, _skipped) = request_result?;

    let entries = result.get("data").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            let id = entry.get("id").and_then(Value::as_str)?.to_string();
            Some(CodexModelSummary {
                id,
                display_name: entry.get("displayName").and_then(Value::as_str).map(str::to_string),
                is_default: entry.get("isDefault").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect())
}

/// Result of `account/read`. Empirically (`codex-cli 0.153.4`, no auth
/// configured), a logged-out server answers `{"account": null,
/// "requiresOpenaiAuth": true}` rather than erroring — unlike
/// `account/rateLimits/read`/`account/usage/read`, which do return a
/// JSON-RPC error when unauthenticated. This probe only distinguishes
/// logged-in vs. not; it does not parse the shape of a populated `account`
/// object, which has not been observed against a real authenticated
/// session yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexAccountStatus {
    pub authenticated: bool,
    pub requires_openai_auth: bool,
}

/// Spawns, initializes, and calls `account/read`, then kills the probe
/// child.
pub async fn probe_account_read(
    codex_binary: &Path,
    codex_home: &OwnedDirGuard,
    timeout: Duration,
) -> Result<CodexAccountStatus, CodexTransportError> {
    let (mut transport, _initialize_result) =
        spawn_initialized(codex_binary, codex_home, timeout).await?;

    let request_result = transport.request("account/read", serde_json::json!({}), timeout).await;
    kill_and_reap(&mut transport).await;
    let (result, _skipped) = request_result?;

    Ok(CodexAccountStatus {
        authenticated: result.get("account").is_some_and(|v| !v.is_null()),
        requires_openai_auth: result
            .get("requiresOpenaiAuth")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Result of `thread/start`. Only the fields needed to identify and later
/// address the thread are modeled; the real response also carries
/// `sessionId`, `path` (the on-disk rollout `.jsonl` under `codex_home`),
/// `model`, `cwd`, and more that have no consumer yet. `turn/start`'s own
/// lifecycle, run on a thread started this way, is modeled separately by
/// `probe_turn_to_completion` below (see module doc).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexThreadSummary {
    pub id: String,
    pub status: String,
}

/// Spawns, initializes, and calls `thread/start` with no parameters (a
/// fresh, unparented thread — `thread/start`'s `params` also accepts
/// `cwd`/`model`/`forkFromId` and more, none of which this probe needs),
/// then kills the probe child. Empirically (`codex-cli 0.153.4`) this
/// succeeds without any account configured, creating a real
/// `status: "idle"` thread and rollout file under `codex_home`.
pub async fn probe_thread_start(
    codex_binary: &Path,
    codex_home: &OwnedDirGuard,
    timeout: Duration,
) -> Result<CodexThreadSummary, CodexTransportError> {
    let (mut transport, _initialize_result) =
        spawn_initialized(codex_binary, codex_home, timeout).await?;

    let request_result = transport.request("thread/start", serde_json::json!({}), timeout).await;
    kill_and_reap(&mut transport).await;
    let (result, _skipped) = request_result?;

    let thread = result.get("thread").cloned().unwrap_or(Value::Null);
    let id = thread
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| CodexTransportError::UnexpectedEnvelope { line: thread.to_string() })?
        .to_string();
    let status = thread
        .get("status")
        .and_then(|s| s.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    Ok(CodexThreadSummary { id, status })
}

/// Terminal outcome of one `turn/start`, taken from the `turn/completed`
/// notification's `params.turn` object (see module doc for the confirmed
/// shape). Only `id`/`status`/`error.message` are parsed — the notification
/// also carries `items`, `startedAt`/`completedAt`/`durationMs`, none of
/// which have a consumer yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexTurnOutcome {
    pub turn_id: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
}

/// Spawns, initializes, starts a fresh thread, then drives one `turn/start`
/// on it all the way to its terminal `turn/completed` notification, then
/// kills the probe child. `turn_completion_timeout` must budget for the
/// full notification window after `turn/start`'s own (immediate,
/// `status:"inProgress"`) response — confirmed empirically this can be
/// 30-40 seconds for the one failure mode observed (repeated WS/HTTPS
/// reconnect attempts against a 401, see module doc), not just a fast
/// round trip.
pub async fn probe_turn_to_completion(
    codex_binary: &Path,
    codex_home: &OwnedDirGuard,
    text: &str,
    turn_completion_timeout: Duration,
) -> Result<CodexTurnOutcome, CodexTransportError> {
    let (mut transport, _initialize_result) =
        spawn_initialized(codex_binary, codex_home, Duration::from_secs(10)).await?;

    let thread_id = match transport
        .request("thread/start", serde_json::json!({}), Duration::from_secs(10))
        .await
    {
        Ok((result, _skipped)) => {
            match result.get("thread").and_then(|t| t.get("id")).and_then(Value::as_str) {
                Some(id) => id.to_string(),
                None => {
                    kill_and_reap(&mut transport).await;
                    return Err(CodexTransportError::UnexpectedEnvelope {
                        line: result.to_string(),
                    });
                }
            }
        }
        Err(err) => {
            kill_and_reap(&mut transport).await;
            return Err(err);
        }
    };

    let turn_start_params = serde_json::json!({
        "threadId": thread_id,
        "input": [{"type": "text", "text": text}],
    });
    if let Err(err) =
        transport.request("turn/start", turn_start_params, Duration::from_secs(10)).await
    {
        kill_and_reap(&mut transport).await;
        return Err(err);
    }

    let completion = transport
        .read_until_notification("turn/completed", turn_completion_timeout)
        .await;
    kill_and_reap(&mut transport).await;
    let (value, _skipped) = completion?;

    let turn = value.get("params").and_then(|p| p.get("turn")).cloned().unwrap_or(Value::Null);
    Ok(CodexTurnOutcome {
        turn_id: turn.get("id").and_then(Value::as_str).map(str::to_string),
        status: turn.get("status").and_then(Value::as_str).unwrap_or("unknown").to_string(),
        error_message: turn
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn real_codex_home() -> OwnedDirGuard {
        let root = std::env::temp_dir().join(format!(
            "automed-codex-transport-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut perms = std::fs::metadata(&root).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o700);
        std::fs::set_permissions(&root, perms).unwrap();
        crate::fs_guard::create_owned_dir(&root, "codex-home").unwrap()
    }

    /// Real-binary integration test: a genuine `codex app-server` speaks
    /// the exact newline-delimited JSON-RPC framing this module assumes,
    /// and `initialize` returns a `userAgent` field.
    #[tokio::test]
    async fn probe_initialize_round_trips_against_the_real_codex_binary() {
        let codex_home = real_codex_home();
        let result = probe_initialize(
            Path::new("/opt/homebrew/bin/codex"),
            &codex_home,
            Duration::from_secs(10),
        )
        .await
        .unwrap();

        assert!(result.user_agent.is_some(), "expected a userAgent in the initialize result");
        assert!(
            result.user_agent.as_deref().unwrap().contains("autome"),
            "expected our clientInfo.name to be reflected in userAgent, got {:?}",
            result.user_agent
        );
        std::fs::remove_dir_all(codex_home.canonical_path.parent().unwrap()).ok();
    }

    /// Real-binary integration test: `model/list` succeeds without any
    /// account configured and returns at least one model whose `id` is
    /// non-empty and exactly one of them is marked `isDefault`.
    #[tokio::test]
    async fn probe_model_list_round_trips_against_the_real_codex_binary() {
        let codex_home = real_codex_home();
        let models = probe_model_list(
            Path::new("/opt/homebrew/bin/codex"),
            &codex_home,
            Duration::from_secs(10),
        )
        .await
        .unwrap();

        assert!(!models.is_empty(), "expected at least one model");
        assert!(models.iter().all(|m| !m.id.is_empty()));
        assert_eq!(
            models.iter().filter(|m| m.is_default).count(),
            1,
            "expected exactly one default model, got {models:?}"
        );
        std::fs::remove_dir_all(codex_home.canonical_path.parent().unwrap()).ok();
    }

    /// Real-binary integration test: `account/read` against a fresh,
    /// never-logged-in `CODEX_HOME` reports `requiresOpenaiAuth: true` and
    /// no authenticated account — this is the oracle a fresh sandboxed
    /// `CODEX_HOME` should always produce, not a guess.
    #[tokio::test]
    async fn probe_account_read_round_trips_against_the_real_codex_binary() {
        let codex_home = real_codex_home();
        let status = probe_account_read(
            Path::new("/opt/homebrew/bin/codex"),
            &codex_home,
            Duration::from_secs(10),
        )
        .await
        .unwrap();

        assert!(!status.authenticated, "a fresh CODEX_HOME must not already be authenticated");
        assert!(status.requires_openai_auth);
        std::fs::remove_dir_all(codex_home.canonical_path.parent().unwrap()).ok();
    }

    /// Real-binary integration test: `thread/start` succeeds without any
    /// account configured, returns a non-empty thread id, and the thread
    /// starts out idle.
    #[tokio::test]
    async fn probe_thread_start_round_trips_against_the_real_codex_binary() {
        let codex_home = real_codex_home();
        let thread = probe_thread_start(
            Path::new("/opt/homebrew/bin/codex"),
            &codex_home,
            Duration::from_secs(10),
        )
        .await
        .unwrap();

        assert!(!thread.id.is_empty());
        assert_eq!(thread.status, "idle");
        std::fs::remove_dir_all(codex_home.canonical_path.parent().unwrap()).ok();
    }

    /// Real-binary integration test: `turn/start` on a thread from an
    /// unauthenticated `CODEX_HOME` resolves via the real `turn/completed`
    /// notification (not a fast JSON-RPC error) as a failed turn carrying
    /// the underlying 401 — the one lifecycle this module documents (see
    /// module doc). Budgets a generous timeout for the confirmed 30-40s
    /// reconnect/backoff window.
    #[tokio::test]
    async fn probe_turn_to_completion_round_trips_the_unauthenticated_failure_mode_against_the_real_codex_binary(
    ) {
        let codex_home = real_codex_home();
        let outcome = probe_turn_to_completion(
            Path::new("/opt/homebrew/bin/codex"),
            &codex_home,
            "say hi",
            Duration::from_secs(90),
        )
        .await
        .unwrap();

        assert!(outcome.turn_id.is_some_and(|id| !id.is_empty()));
        assert_eq!(outcome.status, "failed");
        assert!(
            outcome.error_message.is_some_and(|msg| msg.contains("401")),
            "expected the terminal turn error to mention the underlying 401"
        );
        std::fs::remove_dir_all(codex_home.canonical_path.parent().unwrap()).ok();
    }

    /// A stand-in binary that never writes a line and never exits must be
    /// killed and reaped, not left running, and `request` must return
    /// promptly rather than waiting out the fixture's own much longer sleep.
    #[tokio::test]
    async fn a_hung_server_times_out_and_is_killed_not_leaked() {
        let codex_home = real_codex_home();
        let started = std::time::Instant::now();
        let err = probe_initialize(
            &fixture("fake_harness_hangs.sh"),
            &codex_home,
            Duration::from_millis(200),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, CodexTransportError::Timeout { .. }));
        assert!(started.elapsed() < Duration::from_secs(5));
        std::fs::remove_dir_all(codex_home.canonical_path.parent().unwrap()).ok();
    }

    /// A stand-in binary whose first line is not JSON must surface a typed
    /// parse error, never panic or hang waiting for a well-formed line that
    /// will never arrive.
    #[tokio::test]
    async fn a_garbage_first_line_yields_a_typed_parse_error() {
        let codex_home = real_codex_home();
        let err = probe_initialize(
            &fixture("fake_codex_garbage.sh"),
            &codex_home,
            Duration::from_secs(2),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, CodexTransportError::MalformedLine(_)));
        std::fs::remove_dir_all(codex_home.canonical_path.parent().unwrap()).ok();
    }

    /// A stand-in binary that exits immediately without writing anything
    /// must surface `ClosedStdout`, not hang or panic on EOF.
    #[tokio::test]
    async fn a_closed_stdout_before_any_response_is_reported_not_hung() {
        let codex_home = real_codex_home();
        let err = probe_initialize(
            &fixture("fake_codex_closes_immediately.sh"),
            &codex_home,
            Duration::from_secs(2),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, CodexTransportError::ClosedStdout));
        std::fs::remove_dir_all(codex_home.canonical_path.parent().unwrap()).ok();
    }

    /// D8 rejection: `spawn`/`probe_initialize` take an `OwnedDirGuard`, not
    /// a bare path, and that guard cannot be constructed for a group/world-
    /// writable directory — `verify_owned_dir` rejects it first. There is no
    /// bypass path from an unsafe `CODEX_HOME` into a spawned `codex`
    /// process; this test proves the rejection happens before spawn is even
    /// reachable, not just that `fs_guard` has its own passing unit tests.
    #[test]
    fn an_unsafe_codex_home_never_produces_a_guard_spawn_could_accept() {
        let root = std::env::temp_dir().join(format!(
            "automed-codex-transport-test-unsafe-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut perms = std::fs::metadata(&root).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o700);
        std::fs::set_permissions(&root, perms).unwrap();

        let unsafe_home = root.join("world-writable-codex-home");
        std::fs::create_dir(&unsafe_home).unwrap();
        let mut perms = std::fs::metadata(&unsafe_home).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o777);
        std::fs::set_permissions(&unsafe_home, perms).unwrap();

        let err = crate::fs_guard::verify_owned_dir(&unsafe_home, &root).unwrap_err();
        assert!(matches!(
            err,
            crate::fs_guard::FsGuardError::GroupOrWorldWritable { .. }
        ));
        std::fs::remove_dir_all(&root).ok();
    }

    /// `classify` treats an object whose `id` doesn't match as `Unmatched`
    /// rather than erroring — server-originated traffic is real, expected
    /// protocol behavior, not malformed input.
    #[test]
    fn classify_distinguishes_matching_response_from_unmatched_traffic() {
        let matching = serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": {}});
        let other_id = serde_json::json!({"jsonrpc": "2.0", "id": 2, "result": {}});
        let notification = serde_json::json!({"jsonrpc": "2.0", "method": "codex/event", "params": {}});

        assert!(matches!(classify(&matching, 1), CodexFrame::Response(_)));
        assert!(matches!(classify(&other_id, 1), CodexFrame::Unmatched(_)));
        assert!(matches!(classify(&notification, 1), CodexFrame::Unmatched(_)));
    }
}
