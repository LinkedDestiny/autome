//! Reading what a session cost out of the CLI's own event stream.
//!
//! Both CLIs have been writing this all along and nobody was reading it. The
//! ledger recorded that a session happened and what it exited with; the
//! question "did that protocol change help" therefore had no answer, which
//! made the whole idea of improving the protocol from evidence decorative.
//!
//! The shapes below were read off real runs on 2026-09-17 against Claude Code
//! 2.1.261 and Codex 0.153.4, not from documentation. Two details that only
//! show up that way:
//!
//! - **Claude repeats a message's usage on every content block.** A single
//!   assistant message with a `thinking` block and a `text` block emits two
//!   `assistant` events carrying the same `message.id` and the same `usage`.
//!   Averaging without de-duplicating by message id would weight long messages
//!   more heavily and quietly inflate `mean_request_input`.
//! - **Codex reports one turn per `codex exec`.** `turn.completed` fires once,
//!   however many tools the agent ran. So Codex turn counts are comparable
//!   between Codex sessions and *not* with Claude's `num_turns`, which counts
//!   the agent loop. Anything comparing across runtimes has to use tokens.
//!
//! Every field is optional and nothing is defaulted to zero. A stream that
//! could not be parsed records nothing, because zero is a claim about a
//! session and absence is not.

use autome_domain::metrics::SessionMetrics;
use autome_domain::role::Runtime;
use serde_json::Value;

/// Parses a session's raw `.jsonl` into usage.
///
/// `wall_ms` is used only where the CLI does not report a duration of its own.
pub fn parse(runtime: Runtime, stream: &str, wall_ms: Option<u64>) -> SessionMetrics {
    match runtime {
        Runtime::Claude => parse_claude(stream),
        Runtime::Codex => parse_codex(stream, wall_ms),
    }
}

/// A failure that is the CLI's provider saying "later", not the round going
/// wrong. Read off the same stream as the usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transient {
    /// The provider's own words, trimmed, for the event and the panel.
    pub message: String,
    /// When the provider said the limit resets, as epoch seconds, if it said.
    pub reset_at: Option<i64>,
}

/// Whether a failed session failed because of a rate or usage limit.
///
/// Only the error-shaped parts of the stream are read — a `result` event
/// marked as an error, Codex's `turn.failed` and `error` events, and the
/// non-JSON lines the wrapper merged in from stderr. Everything else is the
/// round's own work, where "429" is as likely to be a line number as a status.
///
/// The phrases are the ones the two CLIs have printed: Claude's
/// `usage limit reached|<epoch>` and `rate_limit_error`, Codex's
/// `You've hit your usage limit … Try again in 2 hours` and
/// `exceeded retry limit, last status: 429`. A wording this does not know
/// falls through to an ordinary crash, which is retried anyway — just with
/// the retry counted.
pub fn classify_failure(runtime: Runtime, stream: &str, now_epoch: i64) -> Option<Transient> {
    classify_transient(error_texts(runtime, stream), now_epoch)
}

/// Only inspect error events, never model names quoted in normal agent output.
pub fn model_not_found(runtime: Runtime, stream: &str) -> Option<String> {
    error_texts(runtime, stream)
        .into_iter()
        .find(|text| text.to_lowercase().contains("model_not_found"))
}

fn error_texts(runtime: Runtime, stream: &str) -> Vec<String> {
    let mut texts: Vec<String> = Vec::new();
    for line in stream.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with('{') {
            texts.push(trimmed.to_string());
            continue;
        }
        let Ok(Value::Object(o)) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let kind = o.get("type").and_then(Value::as_str).unwrap_or("");
        let pick = |v: Option<&Value>| {
            v.map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| v.to_string())
            })
        };
        match (runtime, kind) {
            (Runtime::Claude, "result") => {
                let is_error = o.get("is_error").and_then(Value::as_bool).unwrap_or(false)
                    || o.get("subtype")
                        .and_then(Value::as_str)
                        .is_some_and(|s| s != "success");
                if is_error {
                    texts.extend(pick(o.get("result")));
                    texts.extend(pick(o.get("error")));
                }
            }
            (_, "error") => {
                texts.extend(pick(o.get("message")));
                texts.extend(pick(o.get("error")));
            }
            (Runtime::Codex, "turn.failed") => {
                texts.extend(pick(o.get("error").and_then(|e| e.get("message"))));
            }
            _ => {}
        }
    }

    texts
}

fn classify_transient(texts: Vec<String>, now_epoch: i64) -> Option<Transient> {
    const PHRASES: [&str; 12] = [
        "rate limit",
        "rate_limit",
        "ratelimit",
        "too many requests",
        "usage limit",
        "hit your limit",
        "limit reached",
        "quota exceeded",
        "overloaded",
        "status: 429",
        " 429 ",
        "error: 429",
    ];
    let hit = texts.into_iter().find(|t| {
        let lower = format!(" {} ", t.to_lowercase());
        PHRASES.iter().any(|p| lower.contains(p))
    })?;
    let reset_at = reset_epoch(&hit, now_epoch);
    let mut message: String = hit.chars().take(300).collect();
    if message.len() < hit.len() {
        message.push('…');
    }
    Some(Transient { message, reset_at })
}

/// Reads a reset time out of a limit message: Claude's `…|1731000000`, or a
/// relative `try again in 2 hours 13 minutes`.
fn reset_epoch(text: &str, now: i64) -> Option<i64> {
    if let Some((_, tail)) = text.rsplit_once('|') {
        let digits: String = tail
            .trim()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(epoch) = digits.parse::<i64>()
            && epoch > now
        {
            return Some(epoch);
        }
    }
    let lower = text.to_lowercase();
    let after = lower.split("try again in").nth(1)?;
    let mut secs = 0i64;
    let words: Vec<&str> = after
        .split(|c: char| c.is_whitespace() || c == ',' || c == '.')
        .filter(|w| !w.is_empty())
        .take(8)
        .collect();
    for pair in words.windows(2) {
        let Ok(n) = pair[0].parse::<i64>() else {
            continue;
        };
        let unit = pair[1];
        secs += if unit.starts_with("hour") || unit == "h" {
            n * 3600
        } else if unit.starts_with("min") || unit == "m" {
            n * 60
        } else if unit.starts_with("sec") || unit == "s" {
            n
        } else if unit.starts_with("day") {
            n * 86_400
        } else {
            0
        };
    }
    (secs > 0).then_some(now + secs)
}

fn objects(stream: &str) -> impl Iterator<Item = serde_json::Map<String, Value>> + '_ {
    stream.lines().filter_map(|line| {
        let line = line.trim();
        if !line.starts_with('{') {
            // The wrapper merges stderr into the same pipe, so a CLI warning
            // sits in here too. Skipping non-JSON is not a tolerance for
            // corruption; it is the format.
            return None;
        }
        match serde_json::from_str::<Value>(line) {
            Ok(Value::Object(o)) => Some(o),
            _ => None,
        }
    })
}

fn u64_at(v: &Value, path: &[&str]) -> Option<u64> {
    let mut cur = v;
    for key in path {
        cur = cur.get(key)?;
    }
    cur.as_u64()
}

fn parse_claude(stream: &str) -> SessionMetrics {
    let mut m = SessionMetrics::default();

    // Mean input per model request, de-duplicated by message id.
    let mut seen: Vec<String> = Vec::new();
    let mut request_inputs: Vec<u64> = Vec::new();

    for o in objects(stream) {
        match o.get("type").and_then(Value::as_str) {
            Some("assistant") => {
                let Some(message) = o.get("message") else {
                    continue;
                };
                let id = message
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if !id.is_empty() {
                    if seen.contains(&id) {
                        continue;
                    }
                    seen.push(id);
                }
                let Some(usage) = message.get("usage") else {
                    continue;
                };
                // "Input for this request" is everything the model had to be
                // given, cache hits included — that is the number a context
                // change moves.
                let total = u64_at(usage, &["input_tokens"]).unwrap_or(0)
                    + u64_at(usage, &["cache_read_input_tokens"]).unwrap_or(0)
                    + u64_at(usage, &["cache_creation_input_tokens"]).unwrap_or(0);
                request_inputs.push(total);
            }
            Some("result") => {
                m.cost_usd = o.get("total_cost_usd").and_then(Value::as_f64);
                m.turns = o.get("num_turns").and_then(Value::as_u64);
                // `duration_api_ms` is time spent waiting on the model;
                // `duration_ms` includes tool execution. The former is the one
                // that moves when the prompt changes.
                m.duration_ms = o
                    .get("duration_api_ms")
                    .and_then(Value::as_u64)
                    .or_else(|| o.get("duration_ms").and_then(Value::as_u64));
                if let Some(usage) = o.get("usage") {
                    m.input_tokens = u64_at(usage, &["input_tokens"]);
                    m.cache_read_tokens = u64_at(usage, &["cache_read_input_tokens"]);
                    m.cache_write_tokens = u64_at(usage, &["cache_creation_input_tokens"]);
                    m.output_tokens = u64_at(usage, &["output_tokens"]);
                }
            }
            _ => {}
        }
    }

    if !request_inputs.is_empty() {
        let sum: u64 = request_inputs.iter().sum();
        m.mean_request_input = Some(sum / request_inputs.len() as u64);
    }
    m
}

fn parse_codex(stream: &str, wall_ms: Option<u64>) -> SessionMetrics {
    let mut m = SessionMetrics::default();
    let mut turns = 0u64;
    let mut seen_usage = false;
    let (mut input, mut cache_read, mut cache_write, mut output) = (0u64, 0u64, 0u64, 0u64);

    for o in objects(stream) {
        if o.get("type").and_then(Value::as_str) != Some("turn.completed") {
            continue;
        }
        turns += 1;
        let Some(usage) = o.get("usage") else {
            continue;
        };
        seen_usage = true;
        // Codex's `input_tokens` is the whole prompt, cached part included;
        // Claude's is the uncached remainder. Subtract so the two columns mean
        // the same thing.
        let total_input = u64_at(usage, &["input_tokens"]).unwrap_or(0);
        let cached = u64_at(usage, &["cached_input_tokens"]).unwrap_or(0);
        input += total_input.saturating_sub(cached);
        cache_read += cached;
        cache_write += u64_at(usage, &["cache_write_input_tokens"]).unwrap_or(0);
        // Reasoning tokens are billed as output and are not included in
        // `output_tokens`; leaving them out would under-report a high-effort
        // session by most of what it produced.
        output += u64_at(usage, &["output_tokens"]).unwrap_or(0)
            + u64_at(usage, &["reasoning_output_tokens"]).unwrap_or(0);
    }

    if turns > 0 {
        m.turns = Some(turns);
    }
    if seen_usage {
        m.input_tokens = Some(input);
        m.cache_read_tokens = Some(cache_read);
        m.cache_write_tokens = Some(cache_write);
        m.output_tokens = Some(output);
        m.mean_request_input = Some((input + cache_read + cache_write) / turns.max(1));
    }
    // Codex reports no duration and no price. The wall clock is ours to
    // measure; a price is not ours to invent, so `cost_usd` stays absent
    // rather than being computed from a table we would have to maintain.
    m.duration_ms = wall_ms;
    m
}

/// Measures the two file-shaped numbers, which no CLI reports: how big the
/// design document got, and how many evidence files exist. Both are read at
/// reap time, from the worktree the session just left.
pub fn measure_documents(
    worktree: &std::path::Path,
    doc_dir: &str,
    slug: &str,
) -> (Option<u64>, Option<u64>) {
    let design = worktree.join(doc_dir).join(format!("{slug}.md"));
    let bytes = std::fs::metadata(&design).ok().map(|m| m.len());
    let evidence = worktree.join(doc_dir).join("evidence");
    let files = std::fs::read_dir(&evidence).ok().map(|entries| {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .count() as u64
    });
    (bytes, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- classify_failure ---------------------------------------------

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn missing_model_is_only_classified_from_errors() {
        let failure =
            r#"{"type":"turn.failed","error":{"message":"model_not_found: missing model"}}"#;
        assert_eq!(
            model_not_found(Runtime::Codex, failure).as_deref(),
            Some("model_not_found: missing model")
        );
        let output = r#"{"type":"item.completed","item":{"type":"agent_message","text":"model_not_found: missing model"}}"#;
        assert_eq!(model_not_found(Runtime::Codex, output), None);
        let limit = r#"{"type":"turn.failed","error":{"message":"rate limit exceeded"}}"#;
        assert_eq!(model_not_found(Runtime::Codex, limit), None);
        assert!(classify_failure(Runtime::Codex, limit, NOW).is_some());
    }

    #[test]
    fn claudes_usage_limit_result_is_transient_with_its_reset_time() {
        let stream = concat!(
            r#"{"type":"system","subtype":"init"}"#,
            "\n",
            r#"{"type":"result","subtype":"success","is_error":true,"result":"Claude AI usage limit reached|1800003600"}"#,
        );
        let t = classify_failure(Runtime::Claude, stream, NOW).expect("transient");
        assert_eq!(t.reset_at, Some(1_800_003_600));
    }

    #[test]
    fn claudes_api_429_is_transient() {
        let stream = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"API Error: 429 {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\"}}"}"#;
        assert!(classify_failure(Runtime::Claude, stream, NOW).is_some());
    }

    #[test]
    fn codexs_usage_limit_says_when_to_try_again() {
        let stream = r#"{"type":"turn.failed","error":{"message":"You've hit your usage limit. Upgrade to Pro or try again in 2 hours 13 minutes."}}"#;
        let t = classify_failure(Runtime::Codex, stream, NOW).expect("transient");
        assert_eq!(t.reset_at, Some(NOW + 2 * 3600 + 13 * 60));
    }

    #[test]
    fn codexs_exhausted_retries_on_429_is_transient() {
        let stream = concat!(
            "stream error: exceeded retry limit, last status: 429 Too Many Requests\n",
            r#"{"type":"error","message":"exceeded retry limit, last status: 429 Too Many Requests"}"#,
        );
        assert!(classify_failure(Runtime::Codex, stream, NOW).is_some());
    }

    #[test]
    fn a_round_that_merely_mentions_429_in_its_work_is_not_rate_limited() {
        // The tool output is the round's own business: a test named
        // `rate_limit_429` failing is a crash like any other.
        let stream = concat!(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"fixing the rate limit test, status 429 expected"}]}}"#,
            "\n",
            r#"{"type":"result","subtype":"success","is_error":false,"result":"done"}"#,
        );
        assert_eq!(classify_failure(Runtime::Claude, stream, NOW), None);
        let codex = r#"{"type":"item.completed","item":{"type":"command_execution","aggregated_output":"429 Too Many Requests"}}"#;
        assert_eq!(classify_failure(Runtime::Codex, codex, NOW), None);
    }

    /// Captured from Claude Code 2.1.261 on 2026-09-17. Trimmed to the fields
    /// this module reads, with the two-content-block repeat preserved: that
    /// repeat is the reason `mean_request_input` de-duplicates.
    const CLAUDE: &str = r#"⚠ a warning on stderr, not JSON
{"type":"system","subtype":"init","session_id":"s"}
{"type":"assistant","message":{"id":"msg_01","usage":{"input_tokens":9,"cache_creation_input_tokens":52947,"cache_read_input_tokens":0,"output_tokens":4}}}
{"type":"assistant","message":{"id":"msg_01","usage":{"input_tokens":9,"cache_creation_input_tokens":52947,"cache_read_input_tokens":0,"output_tokens":4}}}
{"type":"user","message":{"content":[{"type":"tool_result"}]}}
{"type":"assistant","message":{"id":"msg_02","usage":{"input_tokens":11,"cache_creation_input_tokens":0,"cache_read_input_tokens":52947,"output_tokens":33}}}
{"type":"result","subtype":"success","total_cost_usd":0.0054687,"num_turns":2,"duration_ms":4760,"duration_api_ms":4153,"usage":{"input_tokens":20,"cache_creation_input_tokens":52947,"cache_read_input_tokens":52947,"output_tokens":37}}
"#;

    /// Captured from Codex 0.153.4 on 2026-09-17.
    const CODEX: &str = r#"Reading prompt from stdin...
{"type":"thread.started","thread_id":"01a0"}
{"type":"item.completed","item":{"id":"item_0","type":"error","message":"a deprecation warning"}}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"Hello!"}}
{"type":"turn.completed","usage":{"input_tokens":36111,"cached_input_tokens":12928,"cache_write_input_tokens":0,"output_tokens":6,"reasoning_output_tokens":128}}
"#;

    #[test]
    fn a_claude_stream_yields_cost_turns_and_the_token_split() {
        let m = parse(Runtime::Claude, CLAUDE, None);
        assert_eq!(m.cost_usd, Some(0.0054687));
        assert_eq!(m.turns, Some(2));
        assert_eq!(m.duration_ms, Some(4153));
        assert_eq!(m.input_tokens, Some(20));
        assert_eq!(m.cache_read_tokens, Some(52947));
        assert_eq!(m.cache_write_tokens, Some(52947));
        assert_eq!(m.output_tokens, Some(37));
    }

    #[test]
    fn the_same_message_repeated_per_content_block_counts_once() {
        // Two requests: 9 + 52947 = 52956, and 11 + 52947 = 52958.
        let m = parse(Runtime::Claude, CLAUDE, None);
        assert_eq!(m.mean_request_input, Some((52956 + 52958) / 2));
    }

    #[test]
    fn mean_request_input_is_not_cache_read_over_turns() {
        // The number this replaced. With one cold request and one warm one it
        // is off by half, and on a cache miss it collapses entirely.
        let m = parse(Runtime::Claude, CLAUDE, None);
        let naive = m.cache_read_tokens.unwrap() / m.turns.unwrap();
        assert_ne!(m.mean_request_input, Some(naive));
    }

    #[test]
    fn a_codex_stream_yields_tokens_and_a_turn_count() {
        let m = parse(Runtime::Codex, CODEX, Some(12_000));
        assert_eq!(m.turns, Some(1));
        // 36111 total input, 12928 of it cached.
        assert_eq!(m.input_tokens, Some(36111 - 12928));
        assert_eq!(m.cache_read_tokens, Some(12928));
        assert_eq!(m.cache_write_tokens, Some(0));
        // Reasoning tokens are output tokens.
        assert_eq!(m.output_tokens, Some(6 + 128));
    }

    #[test]
    fn codex_never_reports_a_price_and_none_is_invented() {
        let m = parse(Runtime::Codex, CODEX, Some(12_000));
        assert_eq!(m.cost_usd, None);
    }

    #[test]
    fn codex_duration_comes_from_the_wall_clock_because_the_cli_reports_none() {
        assert_eq!(
            parse(Runtime::Codex, CODEX, Some(12_000)).duration_ms,
            Some(12_000)
        );
        assert_eq!(parse(Runtime::Codex, CODEX, None).duration_ms, None);
    }

    #[test]
    fn a_stream_with_no_result_event_records_nothing_rather_than_zero() {
        // A crashed session. Recording zeroes would drag every average it is
        // part of towards a number that describes nothing.
        let truncated: String = CLAUDE
            .lines()
            .filter(|l| !l.contains("\"result\""))
            .collect::<Vec<_>>()
            .join("\n");
        let m = parse(Runtime::Claude, &truncated, None);
        assert_eq!(m.cost_usd, None);
        assert_eq!(m.turns, None);
        assert_eq!(m.total_tokens(), None);
        // The per-request mean survives, because the assistant events did.
        assert!(m.mean_request_input.is_some());
    }

    #[test]
    fn an_empty_or_unparseable_stream_yields_nothing() {
        assert!(parse(Runtime::Claude, "", None).is_empty());
        assert!(parse(Runtime::Claude, "not json at all\n", None).is_empty());
        assert!(parse(Runtime::Codex, "not json at all\n", None).is_empty());
    }

    #[test]
    fn a_truncated_last_line_does_not_lose_the_lines_before_it() {
        let mangled = format!("{CLAUDE}{{\"type\":\"resu");
        let m = parse(Runtime::Claude, &mangled, None);
        assert_eq!(m.turns, Some(2));
    }

    #[test]
    fn measuring_documents_reports_absence_rather_than_zero() {
        let dir = std::env::temp_dir().join(format!("autome-usage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("docs/demo")).unwrap();
        let (bytes, files) = measure_documents(&dir, "docs/demo", "demo");
        assert_eq!(bytes, None, "no design document yet");
        assert_eq!(files, None, "no evidence directory yet");

        std::fs::write(dir.join("docs/demo/demo.md"), "abcdef").unwrap();
        std::fs::create_dir_all(dir.join("docs/demo/evidence")).unwrap();
        std::fs::write(dir.join("docs/demo/evidence/M-01-r1-impl.md"), "x").unwrap();
        std::fs::write(dir.join("docs/demo/evidence/M-01-r1-audit.md"), "y").unwrap();
        let (bytes, files) = measure_documents(&dir, "docs/demo", "demo");
        assert_eq!(bytes, Some(6));
        assert_eq!(files, Some(2));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
