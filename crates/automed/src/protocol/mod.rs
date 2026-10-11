//! The Loop protocol: rules, prompt templates and eval cases, compiled in.
//!
//! The rules the six roles run under live in `protocol/` at the root of the
//! autome-v2 repository, and nowhere else. They briefly lived in a git repository of their own
//! at `~/.autome/protocol/` with tags, pinning, rollback and a meta task that
//! edited them; that was removed on 2026-10-11 (see [`current`]). Changing the
//! protocol now means editing `protocol/`, running `automed protocol eval` against
//! it, and shipping a build.
//!
//! Two properties this file exists to hold up:
//!
//! 1. **A running task cannot have its rules changed underneath it.** Task
//!    creation copies the whole protocol into `docs/<slug>/protocol/`, and
//!    sessions read that copy. Protocol principle 6 ("版本固定") stops being a
//!    sentence a session has to obey and becomes a fact about the filesystem —
//!    which is also why a newer build never disturbs a task already running.
//! 2. **A tag is a name; the hash is the identity.** `ProtocolRef` carries
//!    both and compares on the hash, so two builds that call different text by
//!    the same version show up as different instead of hiding.

use std::path::Path;
use std::sync::OnceLock;

use autome_domain::protocol::{ContractRegion, ProtocolFiles, ProtocolRef};

/// Where a task keeps its frozen copy, relative to `docs/<slug>/`.
pub const TASK_SUBDIR: &str = "protocol";

const TAG_PREFIX: &str = "protocol/v";

/// The protocol, compiled into the binary.
///
/// Listed explicitly rather than walked at build time: a file that silently
/// stops being embedded is exactly the kind of thing that only shows up on a
/// user's machine.
const SEED: [(&str, &str); 13] = [
    (
        "loop-protocol.md",
        include_str!("../../../../protocol/loop-protocol.md"),
    ),
    (
        "session-protocol.md",
        include_str!("../../../../protocol/session-protocol.md"),
    ),
    (
        "brief-map.toml",
        include_str!("../../../../protocol/brief-map.toml"),
    ),
    (
        "CHANGELOG.md",
        include_str!("../../../../protocol/CHANGELOG.md"),
    ),
    (
        "prompts/intake.md",
        include_str!("../../../../protocol/prompts/intake.md"),
    ),
    (
        "prompts/onboarding.md",
        include_str!("../../../../protocol/prompts/onboarding.md"),
    ),
    (
        "prompts/plan.md",
        include_str!("../../../../protocol/prompts/plan.md"),
    ),
    (
        "prompts/review.md",
        include_str!("../../../../protocol/prompts/review.md"),
    ),
    (
        "prompts/adjudicate.md",
        include_str!("../../../../protocol/prompts/adjudicate.md"),
    ),
    (
        "prompts/impl.md",
        include_str!("../../../../protocol/prompts/impl.md"),
    ),
    (
        "prompts/audit.md",
        include_str!("../../../../protocol/prompts/audit.md"),
    ),
    (
        "prompts/retro.md",
        include_str!("../../../../protocol/prompts/retro.md"),
    ),
    ("README.md", include_str!("../../../../protocol/README.md")),
];

/// The seed as a version, including the generated `contract.toml`.
///
/// Built once. It is 91 `include_str!` slices into a map, plus a hash of every
/// contract region, plus the rendered `contract.toml` — all deterministic, and
/// `ensure()` calls it on every project add, every pin, every rollback and the
/// first session of every task. `expected_contract()` already memoised the
/// half of it that it needed; this memoises the whole thing so the other
/// callers stop paying too.
pub fn seed() -> &'static ProtocolFiles {
    static CELL: OnceLock<ProtocolFiles> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut files = ProtocolFiles::from_pairs(SEED);
        for (path, content) in eval_seed() {
            files.insert(path, content);
        }
        let toml = render_contract_toml(&contract_regions_of(&files));
        files.insert("contract.toml", toml);
        files
    })
}

/// Eval cases, kept separate from `SEED` only because there are many of them
/// and they change as a group.
fn eval_seed() -> Vec<(&'static str, &'static str)> {
    crate::protocol::evals::SEED.to_vec()
}

pub mod case;
pub mod eval;
pub mod eval_run;
pub mod evals;
pub mod phrases;

fn contract_regions_of(files: &ProtocolFiles) -> Vec<ContractRegion> {
    autome_domain::protocol::regions(files)
        .expect("the seed's contract markers are checked by a unit test")
}

/// The expectation the kernel holds a candidate version to.
///
/// Derived from the seed compiled into this binary, never read from the
/// repository being checked. An expectation stored inside the file it guards
/// can be edited away along with it — that was the hole the 2026-09-17 audit
/// found in the first draft, and `contract.toml` in the repository is a
/// human-readable copy with no authority.
pub fn expected_contract() -> &'static [ContractRegion] {
    static CELL: OnceLock<Vec<ContractRegion>> = OnceLock::new();
    CELL.get_or_init(|| contract_regions_of(seed()))
}

fn render_contract_toml(regions: &[ContractRegion]) -> String {
    let mut s = String::from(
        "# 内核契约区清单。由 Autome 生成，**改这个文件不会改变内核的判断**——\n\
         # 期望哈希存在二进制里，这份只是给人看的。\n\
         #\n\
         # 每一项对应协议文件里一对 `<!-- kernel-contract: 名字 -->` 标记。\n\
         # 标记之间的文本是内核解析的东西：状态块字段、里程碑表列、Backlog 与\n\
         # 争议项的格式、四条守卫条文。改它们要改内核，不能只改协议。\n\n",
    );
    for r in regions {
        s.push_str("[[region]]\n");
        s.push_str(&format!("file = \"{}\"\n", r.file));
        s.push_str(&format!("name = \"{}\"\n", r.name));
        s.push_str(&format!("hash = \"{}\"\n\n", r.hash));
    }
    s
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ProtocolError {
    pub detail: String,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for ProtocolError {}

pub type Result<T> = std::result::Result<T, ProtocolError>;

fn err(detail: impl Into<String>) -> ProtocolError {
    ProtocolError {
        detail: detail.into(),
    }
}

// ---------------------------------------------------------------------------
// The version in force
// ---------------------------------------------------------------------------

/// The protocol every new task runs under: the seed compiled into this binary.
///
/// There used to be a git repository at `~/.autome/protocol/` holding tagged
/// versions, with pinning, rollback and a meta task that edited it. Its
/// release path was never finished — a newer seed only ever landed on a side
/// tag, so `main` sat on v1 while v2 was the version in force, and a meta task
/// branched from `main` would have rewritten v1 and silently dropped v2. The
/// protocol now has one home, the autome-v2 source tree; changing it means
/// editing `protocol/` and shipping a build.
///
/// The tag is the newest `## protocol/vN` heading of the seed's own
/// changelog, so a task's `protocol_ref` still names a version a person can
/// look up, and the hash still says whether two tasks ran the same text.
pub fn current() -> &'static (ProtocolRef, ProtocolFiles) {
    static CELL: OnceLock<(ProtocolRef, ProtocolFiles)> = OnceLock::new();
    CELL.get_or_init(|| {
        let files = seed().clone();
        let tag = files
            .get("CHANGELOG.md")
            .and_then(newest_version_heading)
            .expect("the seed's changelog names its version; a unit test checks it");
        (ProtocolRef::new(tag, files.hash()), files)
    })
}

fn newest_version_heading(changelog: &str) -> Option<String> {
    changelog
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .map(str::trim)
        .filter(|h| {
            h.strip_prefix(TAG_PREFIX)
                .is_some_and(|n| n.parse::<u32>().is_ok())
        })
        .max_by_key(|h| h[TAG_PREFIX.len()..].parse::<u32>().unwrap_or(0))
        .map(str::to_string)
}

/// A scaffold script that arrives without its executable bit is a case that
/// cannot run, and the failure looks like a protocol problem rather than a
/// permissions one. `include_str!` does not carry the mode, so restore it.
fn set_executable_if_script(path: &Path) {
    #[cfg(unix)]
    if path.extension().is_some_and(|e| e == "sh") {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Reads a directory of protocol files, for `autome protocol eval` run against
/// an edited `protocol/` or a task's frozen copy.
pub fn read_dir_protocol(dir: &Path) -> Result<ProtocolFiles> {
    if !dir.is_dir() {
        return Err(err(format!("{} 不是一个目录", dir.display())));
    }
    read_dir_files(dir, dir)
}

/// Reads every file under `root`, skipping `.git`. Paths come back
/// repository-relative with `/` separators.
fn read_dir_files(root: &Path, dir: &Path) -> Result<ProtocolFiles> {
    let mut files = ProtocolFiles::new();
    let entries =
        std::fs::read_dir(dir).map_err(|e| err(format!("无法读取 {}：{e}", dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|e| err(format!("无法读取目录项：{e}")))?;
        let path = entry.path();
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        if path.is_dir() {
            for (p, c) in read_dir_files(root, &path)?.iter() {
                files.insert(p, c);
            }
        } else if let Ok(text) = std::fs::read_to_string(&path) {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| err("协议目录里出现了目录之外的路径"))?
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(rel, text);
        }
    }
    Ok(files)
}

/// Writes a task's frozen copy into `<worktree>/<doc_dir>/protocol/`.
///
/// Returns the repository-relative paths written, for the caller to commit.
pub fn copy_into_task(
    files: &ProtocolFiles,
    worktree: &Path,
    doc_dir: &str,
) -> Result<Vec<String>> {
    let mut written = Vec::new();
    for (path, content) in files.iter() {
        // Eval fixtures are large and a task never reads them; the copy exists
        // so the rules can be reproduced, not so the cases can be re-run.
        if path.starts_with("evals/") {
            continue;
        }
        let rel = format!("{doc_dir}/{TASK_SUBDIR}/{path}");
        let full = worktree.join(&rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| err(format!("无法创建 {}：{e}", parent.display())))?;
        }
        std::fs::write(&full, content)
            .map_err(|e| err(format!("无法写入 {}：{e}", full.display())))?;
        set_executable_if_script(&full);
        written.push(rel);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::protocol::{LOOP_PROTOCOL, SESSION_PROTOCOL, verify_contract};

    #[test]
    fn the_seed_carries_both_protocol_files_and_a_template_per_role() {
        let s = seed().clone();
        assert!(s.loop_protocol().is_some());
        assert!(s.session_protocol().is_some());
        for role in ["plan", "review", "adjudicate", "impl", "audit", "retro"] {
            assert!(s.prompt(role).is_some(), "missing template for {role}");
        }
        assert!(s.prompt("intake").is_some());
        assert!(s.prompt("onboarding").is_some());
    }

    #[test]
    fn the_seed_contract_markers_are_well_formed() {
        // `expected_contract` panics on malformed markers, which would be a
        // panic at first use on a user's machine. Fail here instead.
        let regions = autome_domain::protocol::regions(seed()).expect("seed markers");
        let names: Vec<String> = regions.iter().map(|r| r.key()).collect();
        assert!(
            names.contains(&format!("{SESSION_PROTOCOL}#status-block")),
            "{names:?}"
        );
        assert!(
            names.contains(&format!("{SESSION_PROTOCOL}#milestone-table")),
            "{names:?}"
        );
        assert!(
            names.contains(&format!("{SESSION_PROTOCOL}#boundaries")),
            "{names:?}"
        );
        assert!(
            names.contains(&format!("{LOOP_PROTOCOL}#roles")),
            "{names:?}"
        );
        assert!(
            names.contains(&format!("{LOOP_PROTOCOL}#backlog-disputes")),
            "{names:?}"
        );
    }

    #[test]
    fn the_seed_satisfies_its_own_contract() {
        assert_eq!(verify_contract(expected_contract(), seed()), vec![]);
    }

    #[test]
    fn the_generated_contract_toml_lists_every_region_but_carries_no_authority() {
        let s = seed().clone();
        let toml = s.get("contract.toml").unwrap();
        for r in expected_contract() {
            assert!(toml.contains(&r.hash), "contract.toml misses {}", r.key());
        }
        // Editing the file must not change the verdict.
        let mut edited = s.clone();
        edited.insert("contract.toml", "[[region]]\nfile = \"nope\"\n");
        let breaches = verify_contract(expected_contract(), &edited);
        assert_eq!(breaches, vec![]);
    }

    #[test]
    fn the_protocol_text_fits_the_size_budget() {
        let s = seed().clone();
        assert!(
            s.sized_bytes() <= autome_domain::protocol::SIZE_BUDGET_BYTES,
            "协议正文 {} 字节，超过 {} 的上限",
            s.sized_bytes(),
            autome_domain::protocol::SIZE_BUDGET_BYTES
        );
    }

    #[test]
    fn the_version_in_force_is_the_newest_changelog_heading_over_the_seed() {
        let (r, files) = current();
        assert_eq!(r.tag, "protocol/v1");
        assert_eq!(r.hash, seed().hash());
        assert_eq!(files.hash(), seed().hash());
    }

    #[test]
    fn the_newest_heading_is_chosen_by_number_not_position() {
        let log = "# 记录\n\n## protocol/v10\n\n## protocol/v9\n\n## 关于 C-1\n";
        assert_eq!(newest_version_heading(log).as_deref(), Some("protocol/v10"));
        assert_eq!(newest_version_heading("# 没有版本\n"), None);
    }

    #[test]
    fn a_task_copy_omits_the_eval_fixtures_but_keeps_the_rules() {
        let dir = std::env::temp_dir().join(format!("autome-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let written = copy_into_task(seed(), &dir, "docs/demo").unwrap();
        assert!(written.iter().any(|p| p.ends_with("loop-protocol.md")));
        assert!(written.iter().any(|p| p.ends_with("prompts/impl.md")));
        assert!(!written.iter().any(|p| p.contains("/evals/")));
        // Spelled out rather than built from a helper: these are the paths the
        // seed's prompts name literally, and this is what pins the copy to
        // what a session will actually go looking for.
        assert!(dir.join("docs/demo/protocol/loop-protocol.md").exists());
        assert!(dir.join("docs/demo/protocol/session-protocol.md").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
