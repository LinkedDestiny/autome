//! Renders Claude Code's `stream-json` output into something a person can read.
//!
//! Sessions are headless, so the session log is the only window onto what a
//! round actually did. With `--output-format text` — the default, and what
//! this project shipped first — `claude -p` prints its closing summary and
//! nothing else: a 3 KB log for a round that edited a dozen files, against
//! 300 KB for the same work on Codex, which streams its execution. The user's
//! report was exact: "只有一个启动内容，没有 AI 实际执行的内容".
//!
//! `--output-format stream-json` carries the whole record, one JSON object per
//! line, and is unreadable in a terminal. So the wrapper pipes the CLI through
//! `automed render-stream`, which turns each event into a line and passes
//! anything it cannot parse through untouched — stderr is merged into the same
//! pipe, and a CLI warning must not be swallowed by a renderer that only
//! understands JSON.
//!
//! The raw stream is kept beside the log as `<session>.jsonl`, so nothing is
//! lost to a rendering choice made here.

use std::io::{BufRead, Write};

use serde_json::Value;

/// Reads JSONL on `input`, writes rendered lines to `output`, flushing each so
/// a `tail -f` on the log follows a live session rather than lagging a buffer
/// behind it.
pub fn render_stream<R: BufRead, W: Write>(input: R, output: &mut W) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if let Some(rendered) = render_line(&line) {
            writeln!(output, "{rendered}")?;
        }
        output.flush()?;
    }
    Ok(())
}

/// One input line to zero or one output lines.
///
/// `None` drops the line; only events with nothing to say to a human are
/// dropped, never a line that failed to parse.
pub fn render_line(line: &str) -> Option<String> {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return Some(String::new());
    }
    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        // Not JSON: a CLI warning on stderr, or a format we do not know.
        // Passing it through is the only safe answer — this is the log.
        return Some(trimmed.to_string());
    };
    let Some(object) = value.as_object() else {
        return Some(trimmed.to_string());
    };

    match object.get("type").and_then(Value::as_str) {
        Some("assistant") => render_assistant(object),
        Some("user") => render_user(object),
        Some("result") => Some(render_result(object)),
        Some("system") => render_system(object),
        // An unknown event type still gets a line: a silently dropped event is
        // how a log starts lying about what happened.
        Some(other) => Some(format!("· {other}")),
        None => Some(trimmed.to_string()),
    }
}

fn render_assistant(object: &serde_json::Map<String, Value>) -> Option<String> {
    let content = object
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)?;
    let mut out: Vec<String> = Vec::new();
    for part in content {
        match part.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = part
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if !text.is_empty() {
                    out.push(text.to_string());
                }
            }
            Some("tool_use") => {
                let name = part.get("name").and_then(Value::as_str).unwrap_or("tool");
                out.push(format!("→ {name} {}", tool_summary(name, part)));
            }
            Some("thinking") => out.push("→ (thinking)".to_string()),
            _ => {}
        }
    }
    if out.is_empty() {
        return None;
    }
    Some(out.join("\n"))
}

/// The one-line gist of a tool call. Deliberately the argument that says
/// *what it acted on*, not the whole input: a log full of pasted file contents
/// is as unreadable as one with nothing in it.
fn tool_summary(name: &str, part: &Value) -> String {
    let input = part.get("input");
    let pick = |key: &str| {
        input
            .and_then(|i| i.get(key))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let value = match name {
        "Bash" => pick("command"),
        "Read" | "Write" | "Edit" | "NotebookEdit" => pick("file_path"),
        "Glob" | "Grep" => pick("pattern"),
        "WebFetch" => pick("url"),
        "Task" => pick("description"),
        _ => pick("description")
            .or_else(|| pick("path"))
            .or_else(|| pick("command")),
    };
    truncate(&value.unwrap_or_default(), TOOL_SUMMARY_MAX)
}

/// A tool result arrives as a `user` message. Only the failures are rendered:
/// the successful ones are the file contents the tool just read, and putting
/// those in the log buries the work in its own inputs.
fn render_user(object: &serde_json::Map<String, Value>) -> Option<String> {
    let content = object
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)?;
    for part in content {
        if part.get("type").and_then(Value::as_str) != Some("tool_result") {
            continue;
        }
        if part.get("is_error").and_then(Value::as_bool) == Some(true) {
            let text = tool_result_text(part);
            return Some(format!("  ✗ {}", truncate(&text, TOOL_ERROR_MAX)));
        }
    }
    None
}

fn tool_result_text(part: &Value) -> String {
    match part.get("content") {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|i| i.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

fn render_result(object: &serde_json::Map<String, Value>) -> String {
    let subtype = object
        .get("subtype")
        .and_then(Value::as_str)
        .unwrap_or("result");
    let turns = object.get("num_turns").and_then(Value::as_u64);
    let ms = object.get("duration_ms").and_then(Value::as_u64);
    let mut line = format!("=== {subtype}");
    if let Some(turns) = turns {
        line.push_str(&format!(" · {turns} 轮"));
    }
    if let Some(ms) = ms {
        line.push_str(&format!(" · {:.1}s", ms as f64 / 1000.0));
    }
    line.push_str(" ===");
    line
}

/// `system` events are bookkeeping. `init` is worth a line because it records
/// which model and which tools the round actually got; the rest are not.
fn render_system(object: &serde_json::Map<String, Value>) -> Option<String> {
    if object.get("subtype").and_then(Value::as_str) != Some("init") {
        return None;
    }
    let model = object.get("model").and_then(Value::as_str).unwrap_or("?");
    let mode = object
        .get("permissionMode")
        .and_then(Value::as_str)
        .unwrap_or("?");
    Some(format!("=== model={model} permission-mode={mode} ==="))
}

const TOOL_SUMMARY_MAX: usize = 160;
const TOOL_ERROR_MAX: usize = 400;

/// Truncates on a character boundary, because a byte slice through a multi-byte
/// character panics — and every prompt and path in this project is Chinese.
fn truncate(value: &str, max: usize) -> String {
    let collapsed = value.replace(['\n', '\r'], " ");
    if collapsed.chars().count() <= max {
        return collapsed;
    }
    let kept: String = collapsed.chars().take(max).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(json: &str) -> Option<String> {
        render_line(json)
    }

    #[test]
    fn a_line_that_is_not_json_passes_through_untouched() {
        // stderr is merged into the same pipe. The CLI's own warnings are part
        // of the record, and a renderer that only understands JSON must not
        // eat them.
        let warning = "⚠ claude.ai connectors are disabled because ANTHROPIC_API_KEY ...";
        assert_eq!(line(warning).as_deref(), Some(warning));
        assert_eq!(line("not json at all").as_deref(), Some("not json at all"));
        assert_eq!(line("{ broken").as_deref(), Some("{ broken"));
    }

    #[test]
    fn a_tool_call_says_what_it_acted_on() {
        let out = line(
            r#"{"type":"assistant","message":{"content":[
               {"type":"tool_use","name":"Bash","input":{"command":"swift build"}}]}}"#,
        )
        .unwrap();
        assert_eq!(out, "→ Bash swift build");

        let read = line(
            r#"{"type":"assistant","message":{"content":[
               {"type":"tool_use","name":"Read","input":{"file_path":"/a/b.swift"}}]}}"#,
        )
        .unwrap();
        assert_eq!(read, "→ Read /a/b.swift");
    }

    #[test]
    fn assistant_prose_is_kept_verbatim() {
        let out = line(
            r#"{"type":"assistant","message":{"content":[
               {"type":"text","text":"我先看一下这个脚本。"}]}}"#,
        )
        .unwrap();
        assert_eq!(out, "我先看一下这个脚本。");
    }

    #[test]
    fn a_failed_tool_result_is_shown_and_a_successful_one_is_not() {
        // A successful result is the file the tool just read. Printing those
        // buries the work in its own inputs; a failure is the thing a person
        // opening the log is looking for.
        let failure = line(
            r#"{"type":"user","message":{"content":[
               {"type":"tool_result","is_error":true,"content":"permission denied"}]}}"#,
        );
        assert_eq!(failure.as_deref(), Some("  ✗ permission denied"));

        let success = line(
            r#"{"type":"user","message":{"content":[
               {"type":"tool_result","content":"line 1\nline 2"}]}}"#,
        );
        assert_eq!(success, None);
    }

    #[test]
    fn an_unknown_event_type_still_produces_a_line() {
        // A silently dropped event is how a log starts lying about what
        // happened. A future CLI version will add types this does not know.
        assert_eq!(
            line(r#"{"type":"whatever_is_next"}"#).as_deref(),
            Some("· whatever_is_next")
        );
    }

    #[test]
    fn truncation_does_not_split_a_multibyte_character() {
        // Every prompt and path in this project is Chinese; a byte slice
        // through one panics.
        let long = "中".repeat(TOOL_SUMMARY_MAX + 50);
        let out = line(&format!(
            r#"{{"type":"assistant","message":{{"content":[
               {{"type":"tool_use","name":"Bash","input":{{"command":"{long}"}}}}]}}}}"#
        ))
        .unwrap();
        assert!(out.ends_with('…'), "{out}");
        assert_eq!(out.chars().filter(|c| *c == '中').count(), TOOL_SUMMARY_MAX);
    }

    #[test]
    fn a_newline_inside_a_tool_argument_stays_on_one_line() {
        let out = line(
            r#"{"type":"assistant","message":{"content":[
               {"type":"tool_use","name":"Bash","input":{"command":"a\nb"}}]}}"#,
        )
        .unwrap();
        assert_eq!(out, "→ Bash a b");
    }

    #[test]
    fn the_result_event_reports_how_the_round_ended() {
        let out =
            line(r#"{"type":"result","subtype":"success","num_turns":7,"duration_ms":91500}"#)
                .unwrap();
        assert_eq!(out, "=== success · 7 轮 · 91.5s ===");
    }

    #[test]
    fn the_init_event_records_the_model_and_permission_mode_actually_used() {
        let out = line(
            r#"{"type":"system","subtype":"init","model":"claude-opus-5","permissionMode":"acceptEdits"}"#,
        )
        .unwrap();
        assert_eq!(
            out,
            "=== model=claude-opus-5 permission-mode=acceptEdits ==="
        );
        assert_eq!(line(r#"{"type":"system","subtype":"hook_started"}"#), None);
    }

    #[test]
    fn rendering_a_stream_writes_one_line_per_event() {
        let input = concat!(
            "{\"type\":\"system\",\"subtype\":\"init\",\"model\":\"m\",\"permissionMode\":\"p\"}\n",
            "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"hi\"}]}}\n",
            "{\"type\":\"result\",\"subtype\":\"success\"}\n"
        );
        let mut out = Vec::new();
        render_stream(std::io::BufReader::new(input.as_bytes()), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(
            text,
            "=== model=m permission-mode=p ===\nhi\n=== success ===\n"
        );
    }
}
