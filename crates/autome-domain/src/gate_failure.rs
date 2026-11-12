//! Stable failure identities and per-node consecutive failure decisions; no I/O.
use crate::task::Node;
use sha2::{Digest, Sha256};

pub const TIMEOUT_SUMMARY: &str = "门禁运行超时，被内核终止";

/// Recognize the kernel's final termination line, not a command's exit code.
pub fn kernel_timeout(tail: &str) -> Option<u32> {
    let line = tail.lines().rev().find(|line| !line.trim().is_empty())?;
    let minutes = line
        .strip_prefix("=== Autome：门禁运行超过 ")?
        .strip_suffix(" 分钟，已终止 ===")?;
    if minutes.is_empty() || !minutes.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    minutes.parse::<u32>().ok().filter(|n| *n > 0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    pub id: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct PreviousResult {
    pub ok: bool,
    pub fingerprint: Option<String>,
    pub same_failures: u32,
    pub tripped: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Streak {
    pub count: u32,
    pub tripped: bool,
}

pub fn counts(node: Node) -> bool {
    matches!(node, Node::Gate | Node::MergeGate | Node::FixGate)
}

pub fn streak(prev: Option<&PreviousResult>, fingerprint: &str, limit: u32) -> Streak {
    let count = match prev {
        Some(p) if !p.ok && !p.tripped && p.fingerprint.as_deref() == Some(fingerprint) => {
            p.same_failures.saturating_add(1)
        }
        _ => 1,
    };
    Streak {
        count,
        tripped: count >= limit,
    }
}

fn word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn error_word(word: &str) -> bool {
    matches!(
        word,
        "error" | "Error" | "ERROR" | "fatal" | "FATAL" | "panicked"
    ) || (word.len() > 5 && (word.ends_with("Error") || word.ends_with("Exception")))
}

fn key_line(line: &str) -> bool {
    (line.starts_with("test ") && line.ends_with(" FAILED"))
        || line.split(|c| !word_char(c)).any(error_word)
        || ["Cannot find", "cannot find", "not found", "No such file"]
            .iter()
            .any(|s| line.contains(s))
}

fn clean(line: &str) -> String {
    let mut chars = line.chars().peekable();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out.trim().to_owned()
}

fn normalize(line: &str) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        let c = rest.chars().next().unwrap();
        let prev = line[..i].chars().next_back();
        let boundary = prev.is_none_or(|c| c.is_whitespace() || "\"'([<=,`".contains(c));
        let url = ["http://", "https://", "ws://", "wss://"]
            .iter()
            .any(|p| rest.starts_with(p));
        let windows = rest.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && rest.as_bytes().get(1..3) == Some(b":\\");
        if boundary && (url || rest.starts_with('/') || rest.starts_with("~/") || windows) {
            out.push_str(if url { "<url>" } else { "<path>" });
            let end = rest
                .find(|c: char| c.is_whitespace() || "\"'`)]>,".contains(c))
                .unwrap_or(rest.len());
            i += end;
        } else if word_char(c) && prev.is_none_or(|p| !word_char(p)) {
            let end = rest.find(|c| !word_char(c)).unwrap_or(rest.len());
            let word = &rest[..end];
            if word.len() >= 7
                && word
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
                && word.chars().any(|c| c.is_ascii_digit())
                && word.chars().any(|c| c.is_ascii_alphabetic())
            {
                out.push_str("<hash>");
                i += end;
            } else if c.is_ascii_digit() {
                out.push('#');
                let mut end = c.len_utf8();
                while end < rest.len() {
                    let b = rest.as_bytes()[end];
                    if b.is_ascii_digit()
                        || (b == b'.'
                            && rest.as_bytes().get(end + 1).is_some_and(u8::is_ascii_digit))
                    {
                        end += 1;
                    } else {
                        break;
                    }
                }
                i += end;
            } else {
                out.push_str(word);
                i += end;
            }
        } else {
            out.push(c);
            i += c.len_utf8();
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn identity(material: &str, summary: String) -> Fingerprint {
    Fingerprint {
        id: format!("{:x}", Sha256::digest(material.as_bytes()))[..16].to_owned(),
        summary,
    }
}

pub fn fingerprint(log: &str, exit_code: i32, timed_out: bool) -> Fingerprint {
    if timed_out {
        return identity("timeout", TIMEOUT_SUMMARY.to_owned());
    }
    let lines: Vec<_> = log.lines().map(clean).collect();
    let mut steps = Vec::new();
    let mut keys = Vec::new();
    for line in &lines {
        if let Some(name) = line
            .strip_prefix("FAILED")
            .filter(|s| s.starts_with(char::is_whitespace))
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with('('))
        {
            if !steps.contains(&name) {
                steps.push(name);
            }
        }
    }
    // ci-local runs successful steps after failures. Only use diagnostics from
    // failed sections when their headers are available; keep generic logs working.
    let scoped = lines.iter().any(|line| {
        line.strip_prefix("==> ")
            .is_some_and(|name| steps.contains(&name))
    });
    let mut failed_section = false;
    for line in &lines {
        if let Some(name) = line.strip_prefix("==> ") {
            failed_section = steps.contains(&name);
        } else if (!scoped || failed_section) && key_line(line) {
            keys.push(line.as_str());
        }
    }
    if steps.is_empty() && keys.is_empty() {
        keys = lines
            .iter()
            .rev()
            .filter(|s| !s.is_empty())
            .take(5)
            .map(String::as_str)
            .collect();
        keys.reverse();
    }
    let last = keys
        .iter()
        .rev()
        .find(|line| error_word(line.split(|c| !word_char(c)).next().unwrap_or("")))
        .copied()
        .or_else(|| keys.last().copied())
        .or_else(|| {
            if scoped {
                return None;
            }
            lines
                .iter()
                .rev()
                .find(|s| !s.is_empty())
                .map(String::as_str)
        })
        .unwrap_or("");
    let summary = if steps.is_empty() {
        last.to_owned()
    } else {
        format!("{}：{last}", steps.join("、"))
    };
    // The ellipsis is included in the 160-character display budget.
    let summary = if summary.chars().count() > 160 {
        format!("{}…", summary.chars().take(159).collect::<String>())
    } else {
        summary
    };
    let mut steps: Vec<_> = steps.into_iter().map(normalize).collect();
    steps.sort();
    steps.dedup();
    let mut keys: Vec<_> = keys.into_iter().map(normalize).collect();
    keys.sort();
    keys.dedup();
    identity(
        &format!(
            "exit:{exit_code}\nsteps:{}\nlines:\n{}",
            steps.join("\n"),
            keys.join("\n")
        ),
        summary,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn gate_timeout_reads_kernel_minutes_and_boundaries() {
        for n in [1, 30, u32::MAX] {
            assert_eq!(
                kernel_timeout(&format!(
                    "output\n=== Autome：门禁运行超过 {n} 分钟，已终止 ===\n"
                )),
                Some(n)
            );
        }
        assert_eq!(
            kernel_timeout("=== Autome：门禁运行超过 01 分钟，已终止 ==="),
            Some(1)
        );
    }
    #[test]
    fn gate_timeout_rejects_other_output() {
        for tail in [
            "",
            "test failed",
            "=== Autome：准备命令运行超过 1 分钟，已终止 ===",
            "=== Autome：门禁运行超过 1 分钟，已终止 ===\nlater output",
        ] {
            assert_eq!(kernel_timeout(tail), None);
        }
        for n in ["0", "-1", "+1", "1.5", "", "4294967296"] {
            assert_eq!(
                kernel_timeout(&format!("=== Autome：门禁运行超过 {n} 分钟，已终止 ===")),
                None
            );
        }
    }
    #[test]
    fn gate_timeout_exit_124_alone_is_not_kernel_timeout() {
        assert_eq!(
            kernel_timeout("timeout: command exited with status 124"),
            None
        );
    }

    use super::*;
    fn fp(log: &str) -> Fingerprint {
        fingerprint(log, 1, false)
    }
    fn previous() -> PreviousResult {
        PreviousResult {
            ok: false,
            fingerprint: Some("F".into()),
            same_failures: 2,
            tripped: false,
        }
    }
    #[test]
    fn volatile_details_do_not_change_identity() {
        let a = "Error: /tmp/中文/a '~/old/a' C:\\old\\a https://localhost:123/a ws://host:123/a 14.02s 23:22:08 abc123def";
        let b = "Error: /other/路径/b '~/new/b' D:\\new\\b https://localhost:456/b ws://host:456/b 99.5s 01:02:03 def456abc";
        assert_eq!(fp(a).id, fp(b).id);
    }
    #[test]
    fn different_errors_differ() {
        assert_ne!(fp("Error: missing ws").id, fp("Error: missing zod").id);
        assert_ne!(fingerprint("", 1, false).id, fingerprint("", 2, false).id);
        assert_eq!(
            fp("Error: a\nError: b\nError: a").id,
            fp("Error: b\nError: a").id
        );
    }
    #[test]
    fn different_steps_differ() {
        assert_ne!(
            fp("FAILED Web test\nError: ws").id,
            fp("FAILED Web typecheck\nError: ws").id
        );
        assert_eq!(fp("FAILED (4s)\nError: ws").id, fp("Error: ws").id);
        assert_eq!(
            fp("FAILED B\nFAILED A\nFAILED B").id,
            fp("FAILED A\nFAILED B").id
        );
    }
    #[test]
    fn t14_fixtures() {
        let first = fp(include_str!("../tests/fixtures/gate-t14/run-01.log"));
        assert_eq!(
            first.id,
            fp(include_str!("../tests/fixtures/gate-t14/run-18.log")).id
        );
        assert_ne!(
            first.id,
            fp(include_str!("../tests/fixtures/gate-t14/run-19.log")).id
        );
        assert!(
            first.summary.contains("Cannot find package 'ws'"),
            "{}",
            first.summary
        );
    }
    #[test]
    fn ansi_and_cr_do_not_change_identity() {
        assert_eq!(
            fp("\x1b[31mError: ws\x1b[0m\r\nFAILED Web").id,
            fp("Error: ws\nFAILED Web").id
        );
    }
    #[test]
    fn timeout_has_one_identity() {
        assert_eq!(
            fingerprint("Error: ws", 124, true),
            fingerprint("anything", 1, true)
        );
        assert_ne!(
            fingerprint("", 124, true).id,
            fingerprint("", 124, false).id
        );
    }
    #[test]
    fn fallback_uses_last_five_nonempty_lines() {
        assert_eq!(fp("discard\na\n\nb\nc\nd\ne").id, fp("a\nb\nc\nd\ne").id);
        assert_ne!(fp("gate says no").id, fp("gate says yes").id);
        assert_eq!(fp("").summary, "");
        assert_eq!(fp("  \r\n").id, fp("").id);
        assert_eq!(fp("Error: ws\nan_error_not_a_panic").id, fp("Error: ws").id);
    }
    #[test]
    fn early_errors_survive_later_output() {
        let suffix = vec!["noise"; 400].join("\n");
        assert_ne!(fp(&format!("Error: old\n{suffix}")).id, fp(&suffix).id);
        let suffix = vec!["noise"; 399].join("\n");
        assert_ne!(fp(&format!("Error: boundary\n{suffix}")).id, fp(&suffix).id);
    }
    #[test]
    fn failed_sections_exclude_successful_diagnostics() {
        let log = |case, noise| {
            format!(
                "==> Rust\ntest {case} ... FAILED\nFAILED (1s)\n==> Web\nError: {noise}\nok (1s)\nFAILED Rust"
            )
        };
        assert_eq!(fp(&log("alpha", "one")), fp(&log("alpha", "two")));
        assert_ne!(fp(&log("alpha", "one")).id, fp(&log("beta", "one")).id);
        assert_eq!(
            fp(&log("alpha", "one")).summary,
            "Rust：test alpha ... FAILED"
        );
        assert_eq!(
            fp("==> Rust\nFAILED (1s)\n==> Web\nError: unrelated\nFAILED Rust").summary,
            "Rust："
        );
    }
    #[test]
    fn summary_contains_steps_and_last_error_with_character_limit() {
        assert_eq!(
            fp("Error: first\nTypeError: last\nfile: error secondary\nFAILED Web").summary,
            "Web：TypeError: last"
        );
        for n in [159, 160, 161, 400] {
            let line = format!("Error: {}", "错".repeat(n - 7));
            let summary = fp(&line).summary;
            assert_eq!(summary.chars().count(), n.min(160));
            assert_eq!(summary.ends_with('…'), n > 160);
        }
    }
    #[test]
    fn identifier_digits_are_preserved() {
        assert_ne!(fp("error TS2307").id, fp("error TS7006").id);
        assert_eq!(fp("error line 001").id, fp("error line 999").id);
    }
    #[test]
    fn first_failure_is_one() {
        assert_eq!(
            streak(None, "F", 3),
            Streak {
                count: 1,
                tripped: false
            }
        );
        assert!(streak(None, "", 1).tripped);
    }
    #[test]
    fn matching_failures_accumulate_to_limit() {
        for limit in 1..=10 {
            let mut p = previous();
            p.same_failures = 0;
            for count in 1..=limit {
                let s = streak(Some(&p), "F", limit);
                assert_eq!(
                    s,
                    Streak {
                        count,
                        tripped: count == limit
                    }
                );
                p.same_failures = s.count;
            }
        }
        let mut p = previous();
        p.same_failures = u32::MAX;
        assert_eq!(streak(Some(&p), "F", 10).count, u32::MAX);
    }
    #[test]
    fn passed_or_changed_failure_resets() {
        let mut p = previous();
        assert_eq!(streak(Some(&p), "G", 3).count, 1);
        p.ok = true;
        assert_eq!(streak(Some(&p), "F", 3).count, 1);
    }
    #[test]
    fn legacy_result_resets() {
        let mut p = previous();
        p.fingerprint = None;
        assert_eq!(streak(Some(&p), "F", 3).count, 1);
    }
    #[test]
    fn tripped_result_resets() {
        let mut p = previous();
        p.tripped = true;
        assert_eq!(
            streak(Some(&p), "F", 3),
            Streak {
                count: 1,
                tripped: false
            }
        );
    }
    #[test]
    fn only_three_nodes_count() {
        for node in Node::ALL.into_iter().chain(Node::BUG) {
            assert_eq!(
                counts(node),
                matches!(node, Node::Gate | Node::MergeGate | Node::FixGate)
            );
        }
    }
}
