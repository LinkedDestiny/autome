//! The protocol repository: `~/.autome/protocol/`.
//!
//! The rules the five — now six — roles run under used to be two `&'static
//! str` constants in `init.rs`. That made "change the protocol" a thing only a
//! release could do, which in turn made the retro round's improvement
//! suggestions a dead end: three of them per task, written down, read by
//! nobody, superseded by the next task's three.
//!
//! Here the protocol is a small git repository. It has versions, a changelog,
//! prompt templates, and eval cases that say what a version is supposed to
//! make a session do. A task records which version it ran under and keeps its
//! own copy; a *meta task* — an ordinary Loop task whose project happens to be
//! this repository — is how it changes.
//!
//! Three properties this file exists to hold up:
//!
//! 1. **A running task cannot have its rules changed underneath it.** Task
//!    creation copies the whole protocol into `docs/<slug>/protocol/`, and
//!    sessions read that copy. Protocol principle 6 ("版本固定") stops being a
//!    sentence a session has to obey and becomes a fact about the filesystem.
//! 2. **A tag is a name; the hash is the identity.** Two machines can have
//!    `protocol/v7` pointing at different content. `ProtocolRef` carries both
//!    and compares on the hash, so the difference shows up instead of hiding.
//! 3. **Upgrading the binary never overwrites the user's repository.** A newer
//!    seed lands on its own tag, `protocol/vN-upstream`, for the user to diff
//!    and merge. Their edits are theirs.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use autome_domain::protocol::{ContractRegion, ProtocolFiles, ProtocolRef};

use crate::git;

/// Bumped whenever any seed file changes. Drives the `-upstream` tag an
/// existing repository gets offered after an upgrade.
pub const SEED_VERSION: u32 = 2;

/// Directory under `~/.autome`.
pub const DIRNAME: &str = "protocol";

/// Where a task keeps its frozen copy, relative to `docs/<slug>/`.
pub const TASK_SUBDIR: &str = "protocol";

const TAG_PREFIX: &str = "protocol/v";

/// The files a fresh repository starts from, compiled into the binary.
///
/// Listed explicitly rather than walked at build time: a file that silently
/// stops being embedded is exactly the kind of thing that only shows up on a
/// user's machine.
const SEED: [(&str, &str); 13] = [
    (
        "loop-protocol.md",
        include_str!("seed/loop-protocol.md"),
    ),
    (
        "session-protocol.md",
        include_str!("seed/session-protocol.md"),
    ),
    ("brief-map.toml", include_str!("seed/brief-map.toml")),
    ("CHANGELOG.md", include_str!("seed/CHANGELOG.md")),
    ("prompts/intake.md", include_str!("seed/prompts/intake.md")),
    (
        "prompts/onboarding.md",
        include_str!("seed/prompts/onboarding.md"),
    ),
    ("prompts/plan.md", include_str!("seed/prompts/plan.md")),
    ("prompts/review.md", include_str!("seed/prompts/review.md")),
    (
        "prompts/adjudicate.md",
        include_str!("seed/prompts/adjudicate.md"),
    ),
    ("prompts/impl.md", include_str!("seed/prompts/impl.md")),
    ("prompts/audit.md", include_str!("seed/prompts/audit.md")),
    ("prompts/retro.md", include_str!("seed/prompts/retro.md")),
    ("README.md", include_str!("seed/README.md")),
];

/// The seed as a version, including the generated `contract.toml`.
pub fn seed() -> ProtocolFiles {
    let mut files = ProtocolFiles::from_pairs(SEED);
    for (path, content) in eval_seed() {
        files.insert(path, content);
    }
    let toml = render_contract_toml(&contract_regions_of(&files));
    files.insert("contract.toml", toml);
    files
}

/// Eval cases, kept separate from `SEED` only because there are many of them
/// and they change as a group.
fn eval_seed() -> Vec<(&'static str, &'static str)> {
    crate::protocol::evals::SEED.to_vec()
}

pub mod eval;
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
    CELL.get_or_init(|| contract_regions_of(&seed()))
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

impl From<git::GitError> for ProtocolError {
    fn from(e: git::GitError) -> Self {
        err(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// The repository
// ---------------------------------------------------------------------------

pub fn repo_path(autome_home: &Path) -> PathBuf {
    autome_home.join(DIRNAME)
}

/// What a project runs under: its pin if it has one, the newest tag otherwise.
///
/// Reads the pin from the project's own `.autome/config.toml` rather than
/// taking it as an argument, because the two callers — adding a project and
/// the recovery pass — reach this from places that have a path and not a
/// resolved configuration.
pub fn resolve_for_project(autome_home: &Path, repo: &Path) -> Result<(ProtocolRef, ProtocolFiles)> {
    let pin = crate::config_io::load_project(repo)
        .ok()
        .and_then(|c| c.loop_overrides.protocol);
    ensure(autome_home)?.resolve(pin.as_deref())
}

#[derive(Debug, Clone)]
pub struct Repo {
    pub path: PathBuf,
}

/// The repository, if it already exists.
///
/// Distinct from [`ensure`] because the read half of the IPC surface must not
/// create anything: the read/write split is what lets the desktop shell put a
/// narrower gate in front of reads, and a "read" that initialises a git
/// repository would make that gate decoration. A fresh install with no
/// projects yet honestly has no protocol repository, and the version page can
/// say so.
pub fn open(autome_home: &Path) -> Option<Repo> {
    let path = repo_path(autome_home);
    path.join(".git").exists().then_some(Repo { path })
}

/// Creates the repository if it is not there, and offers an upstream tag if the
/// binary's seed has moved on. Idempotent: the common case is two `git` calls
/// and no writes.
pub fn ensure(autome_home: &Path) -> Result<Repo> {
    let path = repo_path(autome_home);
    let repo = Repo { path: path.clone() };

    if !path.join(".git").exists() {
        std::fs::create_dir_all(&path)
            .map_err(|e| err(format!("无法创建协议仓库目录 {}：{e}", path.display())))?;
        git::init(&path, "main")?;
        write_files(&path, &seed())?;
        commit_all(&path, "chore(protocol): 协议 v1，从二进制内置的种子初始化")?;
        git::run_ok(&path, &["tag", "-f", &format!("{TAG_PREFIX}1")])?;
        return Ok(repo);
    }

    repo.offer_upstream()?;
    Ok(repo)
}

impl Repo {
    /// Released tags, oldest first. Tags that are not `protocol/vN` — including
    /// the `-upstream` ones — are ignored: they are not versions a task can run
    /// under.
    pub fn tags(&self) -> Result<Vec<String>> {
        let out = git::run(&self.path, &["tag", "--list", "protocol/v*"])?;
        let mut tags: Vec<(u32, String)> = out
            .stdout
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .filter_map(|l| Some((version_number(l)?, l.to_string())))
            .collect();
        tags.sort_by_key(|(n, _)| *n);
        Ok(tags.into_iter().map(|(_, t)| t).collect())
    }

    pub fn latest_tag(&self) -> Result<String> {
        self.tags()?
            .pop()
            .ok_or_else(|| err("协议仓库里没有任何 protocol/vN 标签"))
    }

    pub fn next_tag(&self) -> Result<String> {
        let highest = self
            .tags()?
            .iter()
            .filter_map(|t| version_number(t))
            .max()
            .unwrap_or(0);
        Ok(format!("{TAG_PREFIX}{}", highest + 1))
    }

    /// The files at a tag. Read out of the object database rather than the
    /// working tree, so a version stays readable while the user has something
    /// else checked out.
    pub fn files_at(&self, rev: &str) -> Result<ProtocolFiles> {
        let listing = git::run_ok(&self.path, &["ls-tree", "-r", "--name-only", rev])?;
        let mut files = ProtocolFiles::new();
        for path in listing.stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let spec = format!("{rev}:{path}");
            let blob = git::run_ok(&self.path, &["show", &spec])?;
            files.insert(path, blob.stdout);
        }
        Ok(files)
    }

    /// The files as they sit on disk. Used by the meta task's worktree and by
    /// `protocol eval` when checking work in progress.
    pub fn working_files(&self) -> Result<ProtocolFiles> {
        read_dir_files(&self.path, &self.path)
    }

    /// Resolves what a new task should run under.
    ///
    /// `pin` is the project's optional `[loop] protocol = "protocol/v7"`.
    /// Absent, the newest tag wins. A pin naming a tag this machine does not
    /// have is an error rather than a silent fallback — the point of pinning is
    /// that it is honoured.
    pub fn resolve(&self, pin: Option<&str>) -> Result<(ProtocolRef, ProtocolFiles)> {
        let tag = match pin {
            Some(p) if !p.trim().is_empty() => {
                let p = p.trim().to_string();
                if !self.tags()?.contains(&p) {
                    return Err(err(format!(
                        "项目固定了协议版本 `{p}`，但本机的协议仓库里没有这个标签"
                    )));
                }
                p
            }
            _ => self.latest_tag()?,
        };
        let files = self.files_at(&tag)?;
        let hash = files.hash();
        Ok((ProtocolRef::new(tag, hash), files))
    }

    /// Records a new version: commits whatever is staged in the working tree
    /// and tags it.
    pub fn release(&self, message: &str) -> Result<ProtocolRef> {
        commit_all(&self.path, message)?;
        let tag = self.next_tag()?;
        git::run_ok(&self.path, &["tag", &tag])?;
        let files = self.files_at(&tag)?;
        Ok(ProtocolRef::new(tag, files.hash()))
    }

    /// Plan §6.4: rolling back is a *forward* commit.
    ///
    /// Moving a tag would take the eval cases back with the text while leaving
    /// the metrics table pointing at a version whose content had changed under
    /// it. Reverting produces a new version with its own number, its own hash,
    /// and a `retire` entry in the changelog — and the old rows stay true.
    pub fn revert_to(&self, tag: &str) -> Result<ProtocolRef> {
        if !self.tags()?.contains(&tag.to_string()) {
            return Err(err(format!("协议仓库里没有标签 `{tag}`")));
        }
        let files = self.files_at(tag)?;
        // Replace the working tree wholesale: a file added after `tag` has to
        // go, and `checkout tag -- .` would leave it behind.
        for existing in read_dir_files(&self.path, &self.path)?.paths() {
            if !files.contains(existing) {
                let _ = std::fs::remove_file(self.path.join(existing));
            }
        }
        write_files(&self.path, &files)?;
        self.release(&format!("revert(protocol): 回到 {tag} 的内容"))
    }

    /// After a binary upgrade, put the new seed on its own tag so the user can
    /// diff and merge. Never touches `main`.
    fn offer_upstream(&self) -> Result<()> {
        self.offer_upstream_of(&seed(), &format!("{TAG_PREFIX}{SEED_VERSION}-upstream"))
    }

    /// The body of `offer_upstream`, with the seed and tag passed in so a test
    /// can stand in for "the binary was upgraded" without being rebuilt.
    fn offer_upstream_of(&self, seed: &ProtocolFiles, upstream_tag: &str) -> Result<()> {
        let existing = git::run(&self.path, &["tag", "--list", upstream_tag])?;
        if !existing.stdout.trim().is_empty() {
            return Ok(());
        }
        // Nothing to offer if some released version already *is* this seed.
        // On a fresh install that is always true of v1, which is why an
        // unchanged binary never creates one of these tags.
        for tag in self.tags()? {
            if self.files_at(&tag)?.hash() == seed.hash() {
                return Ok(());
            }
        }
        // Built in a throwaway linked worktree. The user's checkout is not
        // touched at all — not even transiently — which matters because they
        // may be part-way through editing the protocol when the app restarts.
        let head = git::head_sha(&self.path).unwrap_or_default();
        if head.is_empty() {
            return Ok(());
        }
        // Outside `.git/`: git declines to register a linked worktree inside
        // the repository's own git directory.
        let scratch = std::env::temp_dir().join(format!(
            "autome-protocol-upstream-{}-{}",
            std::process::id(),
            upstream_tag.replace('/', "-")
        ));
        let scratch_str = scratch.to_string_lossy().to_string();
        let _ = git::run(&self.path, &["worktree", "remove", "--force", &scratch_str]);
        let _ = std::fs::remove_dir_all(&scratch);
        git::run_ok(
            &self.path,
            &["worktree", "add", "--detach", &scratch_str, &head],
        )?;

        let result = (|| -> Result<()> {
            for existing in read_dir_files(&scratch, &scratch)?.paths() {
                if !seed.contains(existing) {
                    let _ = std::fs::remove_file(scratch.join(existing));
                }
            }
            write_files(&scratch, seed)?;
            commit_all(
                &scratch,
                &format!("chore(protocol): 二进制自带的 {upstream_tag} 种子，供对比合并"),
            )?;
            let sha = git::head_sha(&scratch)?;
            git::run_ok(&self.path, &["tag", upstream_tag, &sha])?;
            Ok(())
        })();

        let _ = git::run(&self.path, &["worktree", "remove", "--force", &scratch_str]);
        let _ = std::fs::remove_dir_all(&scratch);
        let _ = git::run(&self.path, &["worktree", "prune"]);
        result
    }
}

fn version_number(tag: &str) -> Option<u32> {
    tag.strip_prefix(TAG_PREFIX)?.parse().ok()
}

fn write_files(root: &Path, files: &ProtocolFiles) -> Result<()> {
    for (path, content) in files.iter() {
        let full = root.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| err(format!("无法创建 {}：{e}", parent.display())))?;
        }
        std::fs::write(&full, content)
            .map_err(|e| err(format!("无法写入 {}：{e}", full.display())))?;
        set_executable_if_script(&full);
    }
    Ok(())
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

fn commit_all(repo: &Path, message: &str) -> Result<()> {
    git::run_ok(repo, &["add", "-A", "."])?;
    let status = git::run(repo, &["diff", "--cached", "--name-only"])?;
    if status.stdout.trim().is_empty() {
        return Ok(());
    }
    git::run_ok(repo, &["commit", "-m", message])?;
    Ok(())
}

/// Reads a directory of protocol files, for `autome protocol eval` run against
/// a checkout that is not a repository this module manages — a meta task's
/// worktree, or a task's frozen copy.
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
    let entries = std::fs::read_dir(dir)
        .map_err(|e| err(format!("无法读取 {}：{e}", dir.display())))?;
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
                .map_err(|_| err("协议仓库里出现了仓库之外的路径"))?
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
pub fn copy_into_task(files: &ProtocolFiles, worktree: &Path, doc_dir: &str) -> Result<Vec<String>> {
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

/// The path a session is pointed at for the loop protocol.
pub fn task_loop_protocol(doc_dir: &str) -> String {
    format!("{doc_dir}/{TASK_SUBDIR}/loop-protocol.md")
}

/// The path a session is pointed at for the session protocol.
pub fn task_session_protocol(doc_dir: &str) -> String {
    format!("{doc_dir}/{TASK_SUBDIR}/session-protocol.md")
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::protocol::{LOOP_PROTOCOL, SESSION_PROTOCOL, verify_contract};

    struct Home(PathBuf);

    impl Home {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "autome-protocol-{tag}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Home(dir)
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn git_available() -> bool {
        git::run(Path::new("."), &["--version"]).is_ok()
    }

    #[test]
    fn the_seed_carries_both_protocol_files_and_a_template_per_role() {
        let s = seed();
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
        let regions = autome_domain::protocol::regions(&seed()).expect("seed markers");
        let names: Vec<String> = regions.iter().map(|r| r.key()).collect();
        assert!(names.contains(&format!("{SESSION_PROTOCOL}#status-block")), "{names:?}");
        assert!(names.contains(&format!("{SESSION_PROTOCOL}#milestone-table")), "{names:?}");
        assert!(names.contains(&format!("{SESSION_PROTOCOL}#boundaries")), "{names:?}");
        assert!(names.contains(&format!("{LOOP_PROTOCOL}#roles")), "{names:?}");
        assert!(
            names.contains(&format!("{LOOP_PROTOCOL}#backlog-disputes")),
            "{names:?}"
        );
    }

    #[test]
    fn the_seed_satisfies_its_own_contract() {
        assert_eq!(verify_contract(expected_contract(), &seed()), vec![]);
    }

    #[test]
    fn the_generated_contract_toml_lists_every_region_but_carries_no_authority() {
        let s = seed();
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
        let s = seed();
        assert!(
            s.sized_bytes() <= autome_domain::protocol::SIZE_BUDGET_BYTES,
            "协议正文 {} 字节，超过 {} 的上限",
            s.sized_bytes(),
            autome_domain::protocol::SIZE_BUDGET_BYTES
        );
    }

    #[test]
    fn ensure_creates_a_tagged_repository_and_is_idempotent() {
        if !git_available() {
            return;
        }
        let home = Home::new("ensure");
        let repo = ensure(&home.0).unwrap();
        assert_eq!(repo.tags().unwrap(), vec!["protocol/v1"]);
        let (r1, files) = repo.resolve(None).unwrap();
        assert_eq!(r1.tag, "protocol/v1");
        assert_eq!(files.hash(), seed().hash());

        let repo2 = ensure(&home.0).unwrap();
        assert_eq!(repo2.tags().unwrap(), vec!["protocol/v1"]);
        assert_eq!(repo2.resolve(None).unwrap().0, r1);
    }

    #[test]
    fn a_user_edit_changes_the_hash_only_after_it_is_released() {
        if !git_available() {
            return;
        }
        let home = Home::new("edit");
        let repo = ensure(&home.0).unwrap();
        let (before, _) = repo.resolve(None).unwrap();

        let path = repo.path.join(LOOP_PROTOCOL);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("{text}\n新增一句，用户自己加的。\n")).unwrap();
        // Uncommitted: tasks still resolve to v1.
        assert_eq!(repo.resolve(None).unwrap().0, before);

        let after = repo.release("feat(protocol): 用户加的一句").unwrap();
        assert_eq!(after.tag, "protocol/v2");
        assert_ne!(after.hash, before.hash);
        assert_eq!(repo.resolve(None).unwrap().0, after);
    }

    #[test]
    fn a_pin_is_honoured_and_an_unknown_pin_is_an_error_rather_than_a_fallback() {
        if !git_available() {
            return;
        }
        let home = Home::new("pin");
        let repo = ensure(&home.0).unwrap();
        let path = repo.path.join(LOOP_PROTOCOL);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("{text}\n二版。\n")).unwrap();
        repo.release("v2").unwrap();

        let (pinned, files) = repo.resolve(Some("protocol/v1")).unwrap();
        assert_eq!(pinned.tag, "protocol/v1");
        assert!(!files.loop_protocol().unwrap().contains("二版。"));
        assert_eq!(repo.resolve(None).unwrap().0.tag, "protocol/v2");

        let e = repo.resolve(Some("protocol/v9")).unwrap_err();
        assert!(e.detail.contains("没有这个标签"), "{}", e.detail);
    }

    #[test]
    fn rolling_back_produces_a_new_version_rather_than_moving_a_tag() {
        if !git_available() {
            return;
        }
        let home = Home::new("revert");
        let repo = ensure(&home.0).unwrap();
        let (v1, _) = repo.resolve(None).unwrap();

        let path = repo.path.join(LOOP_PROTOCOL);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("{text}\n后悔的改动。\n")).unwrap();
        let v2 = repo.release("v2").unwrap();

        let v3 = repo.revert_to("protocol/v1").unwrap();
        assert_eq!(v3.tag, "protocol/v3");
        // Same content as v1, new name: the metrics rows for v2 stay true.
        assert_eq!(v3.hash, v1.hash);
        assert_ne!(v3.tag, v1.tag);
        assert_eq!(repo.tags().unwrap(), vec!["protocol/v1", "protocol/v2", "protocol/v3"]);
        assert_eq!(repo.files_at(&v2.tag).unwrap().hash(), v2.hash);
    }

    #[test]
    fn rolling_back_removes_files_that_the_later_version_added() {
        if !git_available() {
            return;
        }
        let home = Home::new("revert-add");
        let repo = ensure(&home.0).unwrap();
        std::fs::write(repo.path.join("extra.md"), "后加的").unwrap();
        repo.release("v2").unwrap();
        assert!(repo.path.join("extra.md").exists());

        repo.revert_to("protocol/v1").unwrap();
        assert!(!repo.path.join("extra.md").exists());
        assert!(!repo.working_files().unwrap().contains("extra.md"));
    }

    #[test]
    fn an_unchanged_binary_offers_no_upstream_tag() {
        if !git_available() {
            return;
        }
        let home = Home::new("upstream-noop");
        let repo = ensure(&home.0).unwrap();
        // The user edits and releases. The seed did not change, so there is
        // nothing to offer — v1 already is the seed.
        let path = repo.path.join(LOOP_PROTOCOL);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("{text}\n用户自己的条文。\n")).unwrap();
        repo.release("feat(protocol): 用户自己的条文").unwrap();

        ensure(&home.0).unwrap();
        let listed = git::run(&repo.path, &["tag", "--list", "protocol/v*-upstream"]).unwrap();
        assert_eq!(listed.stdout.trim(), "");
    }

    #[test]
    fn a_newer_seed_lands_on_an_upstream_tag_without_touching_the_users_version() {
        if !git_available() {
            return;
        }
        let home = Home::new("upstream");
        let repo = ensure(&home.0).unwrap();
        // The user edits and releases their own v2.
        let path = repo.path.join(LOOP_PROTOCOL);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, format!("{text}\n用户自己的条文。\n")).unwrap();
        let mine = repo.release("feat(protocol): 用户自己的条文").unwrap();

        // The binary is upgraded: a seed that differs from every released
        // version.
        let mut next_seed = seed();
        let upstream_text = format!("{}\n上游新加的条文。\n", next_seed.loop_protocol().unwrap());
        next_seed.insert(LOOP_PROTOCOL, upstream_text);
        let tag = "protocol/v2-upstream";
        repo.offer_upstream_of(&next_seed, tag).unwrap();

        let listed = git::run(&repo.path, &["tag", "--list", tag]).unwrap();
        assert_eq!(listed.stdout.trim(), tag);
        // The user's version is untouched and still what a task resolves to.
        assert_eq!(repo.resolve(None).unwrap().0, mine);
        assert!(
            repo.working_files()
                .unwrap()
                .loop_protocol()
                .unwrap()
                .contains("用户自己的条文")
        );
        // The offered tag is the new seed, and only that.
        assert_eq!(repo.files_at(tag).unwrap().hash(), next_seed.hash());
        // Offering twice is a no-op.
        repo.offer_upstream_of(&next_seed, tag).unwrap();
        assert_eq!(repo.files_at(tag).unwrap().hash(), next_seed.hash());
    }

    #[test]
    fn a_task_copy_omits_the_eval_fixtures_but_keeps_the_rules() {
        let dir = std::env::temp_dir().join(format!("autome-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let written = copy_into_task(&seed(), &dir, "docs/demo").unwrap();
        assert!(written.iter().any(|p| p.ends_with("loop-protocol.md")));
        assert!(written.iter().any(|p| p.ends_with("prompts/impl.md")));
        assert!(!written.iter().any(|p| p.contains("/evals/")));
        assert!(dir.join(task_loop_protocol("docs/demo")).exists());
        assert!(dir.join(task_session_protocol("docs/demo")).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
