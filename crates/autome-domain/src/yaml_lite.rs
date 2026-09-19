//! A deliberately small reader for the two block formats the Loop writes by
//! hand: `lessons.md` and the protocol `CHANGELOG.md`.
//!
//! It is not YAML and does not try to be. Three reasons a real YAML parser is
//! the wrong dependency here:
//!
//! 1. **`#` is not a comment.** Every retro round writes `审计 #3`, and YAML
//!    would eat the rest of the line. The field most worth keeping is the one
//!    that would be truncated.
//! 2. **The schemas are closed.** Six field names in one file, seven in the
//!    other. A reader that knows them can say which field is wrong and on
//!    which line, which is what the core needs in order to hand a session a
//!    correction rather than a stack trace.
//! 3. **The blocks live inside Markdown.** A file is prose with blocks in it,
//!    sometimes fenced and sometimes not, and both forms show up. A YAML
//!    parser would need the same Markdown scanner in front of it anyway.
//!
//! What it understands: a block starts at a `- key: value` bullet and runs
//! through the indented `key: value` lines under it. Values may be bare,
//! single- or double-quoted, an inline flow map `{a: b, c: d}`, or an inline
//! flow sequence `[a, b]`. Nothing else.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Error {
    /// 1-based line in the source document.
    pub line: usize,
    pub detail: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "第 {} 行：{}", self.line, self.detail)
    }
}

impl std::error::Error for Error {}

pub fn err(line: usize, detail: impl Into<String>) -> Error {
    Error {
        line,
        detail: detail.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub line: usize,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Line the bullet started on.
    pub line: usize,
    pub fields: Vec<Field>,
}

impl Block {
    pub fn get(&self, key: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.key == key)
    }

    pub fn require(&self, key: &str) -> Result<&Field, Error> {
        self.get(key)
            .ok_or_else(|| err(self.line, format!("缺少字段 `{key}`")))
    }

    pub fn first_key(&self) -> &str {
        self.fields.first().map(|f| f.key.as_str()).unwrap_or("")
    }

    /// The first field name that appears twice, if any. Repeats are always a
    /// mistake in these files and silently keeping the last one hides it.
    pub fn duplicate(&self) -> Option<&Field> {
        self.fields
            .iter()
            .enumerate()
            .find(|(i, f)| self.fields[..*i].iter().any(|g| g.key == f.key))
            .map(|(_, f)| f)
    }

    /// Rejects a block carrying a field the schema does not define. Without
    /// this a typo'd key is simply ignored and the block quietly means
    /// something else than it reads.
    pub fn reject_unknown(&self, known: &[&str]) -> Result<(), Error> {
        match self
            .fields
            .iter()
            .find(|f| !known.contains(&f.key.as_str()))
        {
            Some(f) => Err(err(f.line, format!("没有 `{}` 这个字段", f.key))),
            None => Ok(()),
        }
    }
}

/// Scans a Markdown document for blocks.
///
/// Infallible by design: prose bullets are normal in these files, and a bullet
/// that is not `key: value` is prose rather than a broken block. Callers
/// select the blocks they want by `first_key()`.
pub fn blocks(doc: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut in_fence = false;
    let mut fence_is_yaml = false;
    let mut current: Option<Block> = None;

    let flush = |current: &mut Option<Block>, out: &mut Vec<Block>| {
        if let Some(b) = current.take()
            && !b.fields.is_empty()
        {
            out.push(b);
        }
    };

    for (i, raw) in doc.lines().enumerate() {
        let line = i + 1;
        let trimmed = raw.trim_start();

        if trimmed.starts_with("```") {
            flush(&mut current, &mut out);
            if in_fence {
                in_fence = false;
                fence_is_yaml = false;
            } else {
                in_fence = true;
                fence_is_yaml = trimmed
                    .trim_start_matches('`')
                    .trim()
                    .eq_ignore_ascii_case("yaml");
            }
            continue;
        }
        // A fence of some other language is a code sample, not data.
        if in_fence && !fence_is_yaml {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("- ") {
            flush(&mut current, &mut out);
            let mut block = Block {
                line,
                fields: Vec::new(),
            };
            if let Some(f) = field(line, rest) {
                block.fields.push(f);
                current = Some(block);
            }
            continue;
        }

        if trimmed.is_empty() {
            // Inside a fence an author may space fields out; outside one, a
            // blank line ends the block.
            if !in_fence {
                flush(&mut current, &mut out);
            }
            continue;
        }

        if current.is_some() {
            if trimmed.starts_with('#') {
                continue;
            }
            // Unindented text ends the block: it is the next paragraph.
            if !(raw.starts_with(' ') || raw.starts_with('\t')) {
                flush(&mut current, &mut out);
                continue;
            }
            match field(line, trimmed) {
                Some(f) => current.as_mut().expect("checked above").fields.push(f),
                // A continuation line that is not `key: value` is appended to
                // the previous value, so a long sentence may be wrapped.
                None => {
                    if let Some(last) = current.as_mut().and_then(|b| b.fields.last_mut()) {
                        last.value.push(' ');
                        last.value.push_str(trimmed);
                    }
                }
            }
        }
    }
    flush(&mut current, &mut out);
    out
}

fn field(line: usize, text: &str) -> Option<Field> {
    let (key, value) = text.split_once(':')?;
    let key = key.trim();
    if key.is_empty() || key.contains(' ') {
        return None;
    }
    Some(Field {
        line,
        key: key.to_string(),
        value: unquote(value.trim()).to_string(),
    })
}

pub fn unquote(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2
        && ((b[0] == b'"' && b[b.len() - 1] == b'"') || (b[0] == b'\'' && b[b.len() - 1] == b'\''))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// `{a: b, c: d}` → `[("a","b"), ("c","d")]`.
pub fn flow_map(line: usize, raw: &str) -> Result<Vec<(String, String)>, Error> {
    let body = raw
        .trim()
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .ok_or_else(|| err(line, format!("`{raw}` 不是 `{{字段: 值, …}}`")))?;
    let mut out = Vec::new();
    for part in body.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (k, v) = part
            .split_once(':')
            .ok_or_else(|| err(line, format!("`{part}` 不是 `字段: 值`")))?;
        out.push((k.trim().to_string(), unquote(v.trim()).to_string()));
    }
    Ok(out)
}

/// `[a, b]` → `["a", "b"]`. A bare value is a one-element sequence, because
/// writing `evidence: voice-schedule L-01` for a single item is what people do.
pub fn flow_seq(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    let body = match trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        Some(b) => b,
        None => trimmed,
    };
    body.split(',')
        .map(|s| unquote(s.trim()).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fenced_block_and_a_bare_block_read_the_same() {
        let fenced = "```yaml\n- id: A\n  x: 1\n```\n";
        let bare = "- id: A\n  x: 1\n";
        assert_eq!(blocks(fenced).len(), 1);
        assert_eq!(blocks(fenced)[0].fields.len(), 2);
        assert_eq!(blocks(bare)[0].fields.len(), 2);
    }

    #[test]
    fn a_hash_in_a_value_is_kept() {
        let b = blocks("- id: A\n  note: 审计 #3 退回\n");
        assert_eq!(b[0].get("note").unwrap().value, "审计 #3 退回");
    }

    #[test]
    fn a_prose_bullet_produces_no_block() {
        assert!(blocks("- 这是一句话，没有冒号\n").is_empty());
    }

    #[test]
    fn a_bullet_whose_colon_is_inside_a_sentence_is_prose() {
        // "注意 这里: 有冒号" — the key would contain a space, which no schema
        // field does.
        assert!(blocks("- 注意 这里: 有冒号\n").is_empty());
    }

    #[test]
    fn a_non_yaml_fence_is_ignored() {
        assert!(blocks("```text\n- id: A\n```\n").is_empty());
    }

    #[test]
    fn a_wrapped_value_is_joined_onto_the_previous_field() {
        let b = blocks("- id: A\n  proposal: 前半句\n    后半句\n");
        assert_eq!(b[0].get("proposal").unwrap().value, "前半句 后半句");
    }

    #[test]
    fn unindented_prose_ends_a_bare_block() {
        let b = blocks("- id: A\n  x: 1\n下面是说明文字\n- id: B\n  x: 2\n");
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].fields.len(), 2);
    }

    #[test]
    fn a_repeated_field_is_reported_rather_than_silently_overwritten() {
        let b = blocks("- id: A\n  x: 1\n  x: 2\n");
        assert_eq!(b[0].duplicate().map(|f| f.key.as_str()), Some("x"));
        assert_eq!(b[0].get("x").unwrap().value, "1");
    }

    #[test]
    fn an_unknown_field_is_rejected_by_name_and_line() {
        let b = blocks("- id: A\n  mood: good\n");
        let e = b[0].reject_unknown(&["id"]).unwrap_err();
        assert_eq!(e.line, 2);
        assert!(e.detail.contains("mood"));
    }

    #[test]
    fn quotes_are_stripped_only_when_they_match_at_both_ends() {
        assert_eq!(unquote("\"a\""), "a");
        assert_eq!(unquote("'a'"), "a");
        assert_eq!(unquote("\"a"), "\"a");
        assert_eq!(unquote("a\" b \"c"), "a\" b \"c");
    }

    #[test]
    fn a_flow_map_reads_its_pairs_in_order() {
        let m = flow_map(1, "{metric: reopen_total, horizon: 3}").unwrap();
        assert_eq!(
            m,
            vec![
                ("metric".to_string(), "reopen_total".to_string()),
                ("horizon".to_string(), "3".to_string())
            ]
        );
    }

    #[test]
    fn something_that_is_not_a_flow_map_says_so() {
        assert!(flow_map(1, "reopen_total").is_err());
    }

    #[test]
    fn a_flow_sequence_accepts_both_the_bracketed_and_the_bare_form() {
        assert_eq!(flow_seq("[a, b]"), vec!["a", "b"]);
        assert_eq!(flow_seq("a"), vec!["a"]);
        assert_eq!(flow_seq("[]"), Vec::<String>::new());
    }
}
