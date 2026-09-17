//! The protocol as versioned data.
//!
//! Until now the Loop protocol was two `&'static str` constants compiled into
//! `automed`. That made it unchangeable by anything except a release, which in
//! turn made "improve the protocol" a thing only a developer could do — and
//! the improvements the Loop itself discovers, round after round, had nowhere
//! to go.
//!
//! This module is the data half of moving it out: a set of files addressed by
//! repository-relative path, a content hash over the whole set, and the
//! *kernel contract* — the handful of regions inside those files that the core
//! parses rather than merely shows to a session. Everything here is pure; the
//! git repository that stores versions lives in `automed::protocol`.
//!
//! Two decisions worth stating, because both were mistakes in an earlier draft:
//!
//! 1. **A version is identified by content hash, not by tag.** A tag is a name
//!    a human reads. Two machines can hold the same tag pointing at different
//!    content, and a task archived under `protocol/v7` has to mean something
//!    six months later. The hash is what a task records.
//! 2. **The expected contract hashes are derived from the seed compiled into
//!    the binary, not read from the files being checked.** An expectation
//!    stored inside the file it guards is not an expectation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Opening marker of a kernel-contract region: `<!-- kernel-contract: name -->`.
pub const CONTRACT_OPEN_PREFIX: &str = "<!-- kernel-contract:";
pub const CONTRACT_CLOSE: &str = "<!-- /kernel-contract -->";

/// The two files every protocol version must carry.
pub const LOOP_PROTOCOL: &str = "loop-protocol.md";
pub const SESSION_PROTOCOL: &str = "session-protocol.md";

/// Which files count towards the size budget checked by `protocol eval`
/// layer 1. Prompt templates and evals are deliberately excluded: the budget
/// is about what a session has to read, and a session reads the protocol.
pub const SIZED_FILES: [&str; 2] = [LOOP_PROTOCOL, SESSION_PROTOCOL];

/// Plan §6.5 layer 1: the protocol text a session reads, in bytes.
pub const SIZE_BUDGET_BYTES: usize = 20 * 1024;

/// What a task records about the protocol it is being held to (plan §3.5).
///
/// `tag` is for humans. `hash` is the identity — comparisons, the version
/// page's grouping, and "did these two machines run the same rules" all use
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtocolRef {
    pub tag: String,
    pub hash: String,
}

impl ProtocolRef {
    pub fn new(tag: impl Into<String>, hash: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            hash: hash.into(),
        }
    }

    /// The single-column form stored in `tasks.protocol_ref` and
    /// `sessions.protocol_ref`: `protocol/v7@3f9a…`.
    pub fn to_wire(&self) -> String {
        format!("{}@{}", self.tag, self.hash)
    }

    pub fn parse(s: &str) -> Option<Self> {
        let (tag, hash) = s.rsplit_once('@')?;
        if tag.is_empty() || hash.is_empty() {
            return None;
        }
        Some(Self::new(tag, hash))
    }

    /// Enough hash to tell two versions apart in a table cell.
    pub fn short_hash(&self) -> &str {
        let n = self.hash.len().min(8);
        &self.hash[..n]
    }
}

impl std::fmt::Display for ProtocolRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.tag, self.short_hash())
    }
}

/// One protocol version's files, keyed by repository-relative path.
///
/// `BTreeMap` rather than `HashMap` so that `hash()` does not need to sort:
/// iteration order *is* the hashed order, and it is the same on every machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProtocolFiles {
    files: BTreeMap<String, String>,
}

impl ProtocolFiles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_pairs<I, P, C>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (P, C)>,
        P: Into<String>,
        C: Into<String>,
    {
        let mut files = BTreeMap::new();
        for (path, content) in pairs {
            files.insert(path.into(), content.into());
        }
        Self { files }
    }

    pub fn insert(&mut self, path: impl Into<String>, content: impl Into<String>) {
        self.files.insert(path.into(), content.into());
    }

    pub fn remove(&mut self, path: &str) -> Option<String> {
        self.files.remove(path)
    }

    pub fn get(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }

    pub fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.files.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The version identity. Length-prefixing both halves is what keeps
    /// `{"ab": "c"}` and `{"a": "bc"}` apart — concatenating path and content
    /// without it would collide.
    pub fn hash(&self) -> String {
        let mut h = Sha256::new();
        for (path, content) in &self.files {
            h.update(path.len().to_le_bytes());
            h.update(path.as_bytes());
            h.update(content.len().to_le_bytes());
            h.update(content.as_bytes());
        }
        hex(&h.finalize())
    }

    /// Bytes of the protocol text a session actually reads (`SIZED_FILES`).
    pub fn sized_bytes(&self) -> usize {
        SIZED_FILES
            .iter()
            .filter_map(|f| self.get(f))
            .map(str::len)
            .sum()
    }

    pub fn loop_protocol(&self) -> Option<&str> {
        self.get(LOOP_PROTOCOL)
    }

    pub fn session_protocol(&self) -> Option<&str> {
        self.get(SESSION_PROTOCOL)
    }

    /// A prompt template by role key (`plan`, `impl`, … plus `intake` and
    /// `onboarding`, which are system steps rather than configurable roles).
    pub fn prompt(&self, name: &str) -> Option<&str> {
        self.get(&format!("prompts/{name}.md"))
    }

    /// Paths under `evals/`, for the eval runner.
    pub fn eval_paths(&self) -> impl Iterator<Item = &str> {
        self.paths().filter(|p| p.starts_with("evals/"))
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

// ---------------------------------------------------------------------------
// The kernel contract
// ---------------------------------------------------------------------------

/// One `<!-- kernel-contract: name -->` … `<!-- /kernel-contract -->` region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractRegion {
    pub file: String,
    pub name: String,
    /// sha256 of the text between the markers, exclusive of both marker lines.
    pub hash: String,
}

impl ContractRegion {
    pub fn key(&self) -> String {
        format!("{}#{}", self.file, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContractError {
    /// A close with no open, or an open that never closes.
    Unbalanced { file: String, detail: String },
    /// Two regions in one file share a name, so neither can be addressed.
    Duplicate { file: String, name: String },
    /// `<!-- kernel-contract: -->` with nothing after the colon.
    Unnamed { file: String, line: usize },
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractError::Unbalanced { file, detail } => {
                write!(f, "{file}：契约标记不成对（{detail}）")
            }
            ContractError::Duplicate { file, name } => {
                write!(f, "{file}：契约区 `{name}` 出现了两次")
            }
            ContractError::Unnamed { file, line } => {
                write!(f, "{file}:{line}：契约标记没有名字")
            }
        }
    }
}

impl std::error::Error for ContractError {}

/// Extracts the contract regions of one file.
///
/// Nesting is not supported and not needed: a second open before the close is
/// reported as unbalanced rather than silently treated as text, because the
/// one thing this check exists to catch is a marker that moved.
pub fn regions_in(file: &str, text: &str) -> Result<Vec<ContractRegion>, ContractError> {
    let mut out: Vec<ContractRegion> = Vec::new();
    let mut open: Option<(String, Vec<&str>)> = None;

    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(CONTRACT_OPEN_PREFIX) {
            if open.is_some() {
                return Err(ContractError::Unbalanced {
                    file: file.to_string(),
                    detail: format!("第 {} 行又开了一个契约区", i + 1),
                });
            }
            let name = rest.trim_end_matches("-->").trim().to_string();
            if name.is_empty() {
                return Err(ContractError::Unnamed {
                    file: file.to_string(),
                    line: i + 1,
                });
            }
            open = Some((name, Vec::new()));
            continue;
        }
        if trimmed == CONTRACT_CLOSE {
            let Some((name, body)) = open.take() else {
                return Err(ContractError::Unbalanced {
                    file: file.to_string(),
                    detail: format!("第 {} 行的闭合标记没有对应的开启标记", i + 1),
                });
            };
            if out.iter().any(|r| r.name == name) {
                return Err(ContractError::Duplicate {
                    file: file.to_string(),
                    name,
                });
            }
            let mut h = Sha256::new();
            h.update(body.join("\n").as_bytes());
            out.push(ContractRegion {
                file: file.to_string(),
                name,
                hash: hex(&h.finalize()),
            });
            continue;
        }
        if let Some((_, body)) = open.as_mut() {
            body.push(line);
        }
    }

    if let Some((name, _)) = open {
        return Err(ContractError::Unbalanced {
            file: file.to_string(),
            detail: format!("契约区 `{name}` 没有闭合"),
        });
    }
    Ok(out)
}

/// Every contract region across a version's files, in `(file, name)` order.
pub fn regions(files: &ProtocolFiles) -> Result<Vec<ContractRegion>, ContractError> {
    let mut out = Vec::new();
    for (path, text) in files.iter() {
        out.extend(regions_in(path, text)?);
    }
    Ok(out)
}

/// A way the contract can be violated. Each variant names its own repair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContractBreach {
    /// The region is gone — deleted, or its markers were removed.
    Missing { file: String, name: String },
    /// The region is still there and its text changed.
    Changed { file: String, name: String },
    /// A region the kernel does not know about. Not fatal on its own, but it
    /// means someone believes the core parses something it does not.
    Unknown { file: String, name: String },
    /// The markers themselves are broken, so nothing can be compared.
    Malformed { detail: String },
}

impl std::fmt::Display for ContractBreach {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractBreach::Missing { file, name } => {
                write!(f, "{file}：契约区 `{name}` 不见了")
            }
            ContractBreach::Changed { file, name } => {
                write!(f, "{file}：契约区 `{name}` 的内容被改了")
            }
            ContractBreach::Unknown { file, name } => {
                write!(f, "{file}：`{name}` 不是内核认识的契约区")
            }
            ContractBreach::Malformed { detail } => f.write_str(detail),
        }
    }
}

/// Compares a candidate version's contract regions against the expectation
/// compiled into the binary.
pub fn verify_contract(expected: &[ContractRegion], files: &ProtocolFiles) -> Vec<ContractBreach> {
    let found = match regions(files) {
        Ok(f) => f,
        Err(e) => {
            return vec![ContractBreach::Malformed {
                detail: e.to_string(),
            }];
        }
    };
    let mut out = Vec::new();
    for want in expected {
        match found.iter().find(|r| r.key() == want.key()) {
            None => out.push(ContractBreach::Missing {
                file: want.file.clone(),
                name: want.name.clone(),
            }),
            Some(got) if got.hash != want.hash => out.push(ContractBreach::Changed {
                file: want.file.clone(),
                name: want.name.clone(),
            }),
            Some(_) => {}
        }
    }
    for got in &found {
        if !expected.iter().any(|w| w.key() == got.key()) {
            out.push(ContractBreach::Unknown {
                file: got.file.clone(),
                name: got.name.clone(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> ProtocolFiles {
        ProtocolFiles::from_pairs([
            (
                LOOP_PROTOCOL,
                "# Loop\n\n<!-- kernel-contract: roles -->\n甲\n乙\n<!-- /kernel-contract -->\n尾巴\n",
            ),
            (SESSION_PROTOCOL, "# Session\n"),
        ])
    }

    #[test]
    fn a_ref_round_trips_through_its_wire_form() {
        let r = ProtocolRef::new("protocol/v7", "abcdef0123");
        assert_eq!(ProtocolRef::parse(&r.to_wire()), Some(r.clone()));
        assert_eq!(r.short_hash(), "abcdef01");
    }

    #[test]
    fn a_tag_containing_an_at_sign_still_parses_because_the_split_is_from_the_right() {
        let r = ProtocolRef::parse("weird@tag@hash").unwrap();
        assert_eq!(r.tag, "weird@tag");
        assert_eq!(r.hash, "hash");
    }

    #[test]
    fn a_ref_without_a_hash_is_not_a_ref() {
        assert_eq!(ProtocolRef::parse("protocol/v7"), None);
        assert_eq!(ProtocolRef::parse("protocol/v7@"), None);
        assert_eq!(ProtocolRef::parse("@hash"), None);
    }

    #[test]
    fn the_hash_changes_when_any_byte_of_any_file_changes() {
        let a = files();
        let before = a.hash();
        let mut b = a.clone();
        b.insert(SESSION_PROTOCOL, "# Session \n");
        assert_ne!(before, b.hash());
    }

    #[test]
    fn moving_a_byte_from_a_path_into_a_neighbouring_file_changes_the_hash() {
        // Without length prefixes these two hash the same.
        let a = ProtocolFiles::from_pairs([("ab", "c"), ("z", "")]);
        let b = ProtocolFiles::from_pairs([("a", "bc"), ("z", "")]);
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn the_hash_is_stable_across_insertion_order() {
        let a = ProtocolFiles::from_pairs([("b", "2"), ("a", "1")]);
        let b = ProtocolFiles::from_pairs([("a", "1"), ("b", "2")]);
        assert_eq!(a.hash(), b.hash());
    }

    #[test]
    fn a_region_hashes_only_the_text_between_its_markers() {
        let r = regions(&files()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].key(), "loop-protocol.md#roles");
        let mut h = Sha256::new();
        h.update("甲\n乙".as_bytes());
        assert_eq!(r[0].hash, hex(&h.finalize()));
    }

    #[test]
    fn text_outside_a_region_can_change_without_changing_the_region_hash() {
        let expected = regions(&files()).unwrap();
        let mut f = files();
        f.insert(
            LOOP_PROTOCOL,
            "# Loop 改了标题\n\n<!-- kernel-contract: roles -->\n甲\n乙\n<!-- /kernel-contract -->\n别的尾巴\n",
        );
        assert_eq!(verify_contract(&expected, &f), vec![]);
    }

    #[test]
    fn editing_inside_a_region_is_a_breach() {
        let expected = regions(&files()).unwrap();
        let mut f = files();
        f.insert(
            LOOP_PROTOCOL,
            "# Loop\n\n<!-- kernel-contract: roles -->\n甲\n丙\n<!-- /kernel-contract -->\n尾巴\n",
        );
        assert_eq!(
            verify_contract(&expected, &f),
            vec![ContractBreach::Changed {
                file: LOOP_PROTOCOL.into(),
                name: "roles".into()
            }]
        );
    }

    #[test]
    fn deleting_the_markers_is_a_breach_even_though_the_text_survives() {
        // This is the case the 2026-09-17 audit raised: an expectation stored
        // in the guarded file can be deleted along with it.
        let expected = regions(&files()).unwrap();
        let mut f = files();
        f.insert(LOOP_PROTOCOL, "# Loop\n\n甲\n乙\n尾巴\n");
        assert_eq!(
            verify_contract(&expected, &f),
            vec![ContractBreach::Missing {
                file: LOOP_PROTOCOL.into(),
                name: "roles".into()
            }]
        );
    }

    #[test]
    fn moving_a_region_to_another_file_is_missing_plus_unknown() {
        let expected = regions(&files()).unwrap();
        let mut f = files();
        f.insert(LOOP_PROTOCOL, "# Loop\n");
        f.insert(
            SESSION_PROTOCOL,
            "<!-- kernel-contract: roles -->\n甲\n乙\n<!-- /kernel-contract -->\n",
        );
        assert_eq!(
            verify_contract(&expected, &f),
            vec![
                ContractBreach::Missing {
                    file: LOOP_PROTOCOL.into(),
                    name: "roles".into()
                },
                ContractBreach::Unknown {
                    file: SESSION_PROTOCOL.into(),
                    name: "roles".into()
                }
            ]
        );
    }

    #[test]
    fn an_unclosed_region_is_an_error_not_a_region_running_to_end_of_file() {
        let e = regions_in("x.md", "<!-- kernel-contract: a -->\n甲\n").unwrap_err();
        assert!(matches!(e, ContractError::Unbalanced { .. }));
    }

    #[test]
    fn a_stray_close_is_an_error() {
        let e = regions_in("x.md", "甲\n<!-- /kernel-contract -->\n").unwrap_err();
        assert!(matches!(e, ContractError::Unbalanced { .. }));
    }

    #[test]
    fn a_nested_open_is_an_error() {
        let e = regions_in(
            "x.md",
            "<!-- kernel-contract: a -->\n<!-- kernel-contract: b -->\n<!-- /kernel-contract -->\n",
        )
        .unwrap_err();
        assert!(matches!(e, ContractError::Unbalanced { .. }));
    }

    #[test]
    fn two_regions_with_the_same_name_in_one_file_are_an_error() {
        let e = regions_in(
            "x.md",
            "<!-- kernel-contract: a -->\n1\n<!-- /kernel-contract -->\n\
             <!-- kernel-contract: a -->\n2\n<!-- /kernel-contract -->\n",
        )
        .unwrap_err();
        assert!(matches!(e, ContractError::Duplicate { .. }));
    }

    #[test]
    fn a_nameless_marker_is_an_error() {
        let e = regions_in("x.md", "<!-- kernel-contract: -->\n1\n<!-- /kernel-contract -->\n")
            .unwrap_err();
        assert!(matches!(e, ContractError::Unnamed { .. }));
    }

    #[test]
    fn malformed_markers_report_once_rather_than_as_every_region_missing() {
        let expected = regions(&files()).unwrap();
        let mut f = files();
        f.insert(LOOP_PROTOCOL, "<!-- kernel-contract: roles -->\n甲\n");
        let breaches = verify_contract(&expected, &f);
        assert_eq!(breaches.len(), 1);
        assert!(matches!(breaches[0], ContractBreach::Malformed { .. }));
    }

    #[test]
    fn sized_bytes_counts_only_the_two_files_a_session_reads() {
        let mut f = files();
        f.insert("prompts/plan.md", "x".repeat(9_000));
        let before = f.sized_bytes();
        f.insert("prompts/impl.md", "y".repeat(9_000));
        assert_eq!(f.sized_bytes(), before);
    }
}
