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

use std::collections::BTreeMap;
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
        write_files(&path, seed())?;
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

    /// Every file in a revision, in two git processes rather than one per
    /// file. Read out of the object database rather than the working tree, so
    /// a version stays readable while the user has something else checked out.
    ///
    /// The first version ran `git show <rev>:<path>` in a loop. The seed is 92
    /// files, so reading one revision cost 93 processes — about 2.6s — and
    /// `resolve()` is on the path of `protocol.get` and `protocol.triggers`,
    /// which the protocol screen calls on arrival. Opening it took five
    /// seconds of spawning.
    ///
    /// `ls-tree -z` for the names and blob ids, then one `cat-file --batch`
    /// fed all the ids at once. `-z` rather than plain `ls-tree` because git
    /// quotes and escapes paths outside ASCII otherwise, and the eval fixtures
    /// are Chinese.
    pub fn files_at(&self, rev: &str) -> Result<ProtocolFiles> {
        // Memoised per (repository, tag). A tag's content cannot change here:
        // `release` and `revert_to` both only ever create a *new* tag, and the
        // one `tag -f` is in `ensure`'s "repository does not exist yet" branch,
        // where no entry can exist. The protocol screen pays for this read
        // three times on arrival — `get`, `triggers` and `eval` all route
        // through `resolve` — and the answer is byte-identical each time.
        //
        // Keyed on the revision string as given, so a caller that passes a SHA
        // or `HEAD` rather than a tag simply misses the cache rather than
        // getting a stale answer for a moving name.
        static CACHE: OnceLock<std::sync::Mutex<BTreeMap<(PathBuf, String), ProtocolFiles>>> =
            OnceLock::new();
        let cache = CACHE.get_or_init(Default::default);
        let key = (self.path.clone(), rev.to_string());
        let cacheable = rev.starts_with(TAG_PREFIX);
        if cacheable && let Ok(map) = cache.lock() && let Some(hit) = map.get(&key) {
            return Ok(hit.clone());
        }

        let listing = git::run_ok(&self.path, &["ls-tree", "-r", "-z", rev])?;
        let mut names: Vec<String> = Vec::new();
        let mut ids = String::new();
        // `<mode> <type> <sha>\t<path>` per NUL-terminated record.
        for record in listing.stdout.split('\0').filter(|r| !r.is_empty()) {
            let Some((meta, path)) = record.split_once('\t') else {
                continue;
            };
            let Some(sha) = meta.split_whitespace().nth(2) else {
                continue;
            };
            names.push(path.to_string());
            ids.push_str(sha);
            ids.push('\n');
        }
        let mut files = ProtocolFiles::new();
        if names.is_empty() {
            return Ok(files);
        }

        let blobs = git::run_ok_with_stdin(&self.path, &["cat-file", "--batch"], &ids)?;
        // `--batch` answers each id with `<sha> <type> <size>\n`, the bytes,
        // and a newline. Walking it by the declared size is the only way that
        // survives a file which itself contains the header shape.
        let bytes = blobs.stdout.as_bytes();
        let mut at = 0usize;
        for name in names {
            let rest = bytes.get(at..).ok_or_else(|| err("cat-file 的输出提前结束"))?;
            let eol = rest
                .iter()
                .position(|b| *b == b'\n')
                .ok_or_else(|| err(format!("cat-file 没有给出 {name} 的头部")))?;
            let header = String::from_utf8_lossy(&rest[..eol]);
            let size: usize = header
                .rsplit(' ')
                .next()
                .and_then(|s| s.trim().parse().ok())
                .ok_or_else(|| err(format!("cat-file 的头部读不出大小：{header}")))?;
            at += eol + 1;
            let content = bytes
                .get(at..at + size)
                .ok_or_else(|| err(format!("cat-file 的 {name} 比声明的短")))?;
            files.insert(&name, String::from_utf8_lossy(content).into_owned());
            at += size + 1;
        }
        if cacheable && let Ok(mut map) = cache.lock() {
            map.insert(key, files.clone());
        }
        Ok(files)
    }

    /// The same file out of several revisions, in one git process.
    ///
    /// `cat-file --batch` takes `<rev>:<path>` object names, so the version
    /// page's "the changelog of every tag" is one spawn rather than one per
    /// tag. A revision that does not have the file is skipped rather than
    /// failing the batch — `--batch` answers those with `<name> missing`, and
    /// an older protocol version legitimately predates a file.
    pub fn file_across(&self, revs: &[String], path: &str) -> Result<BTreeMap<String, String>> {
        let mut out = BTreeMap::new();
        if revs.is_empty() {
            return Ok(out);
        }
        let names: Vec<String> = revs.iter().map(|r| format!("{r}:{path}")).collect();
        let batch = git::run_ok_with_stdin(
            &self.path,
            &["cat-file", "--batch"],
            &format!("{}\n", names.join("\n")),
        )?;
        let bytes = batch.stdout.as_bytes();
        let mut at = 0usize;
        for rev in revs {
            let Some(rest) = bytes.get(at..) else { break };
            let Some(eol) = rest.iter().position(|b| *b == b'\n') else {
                break;
            };
            let header = String::from_utf8_lossy(&rest[..eol]).into_owned();
            at += eol + 1;
            if header.ends_with(" missing") {
                continue;
            }
            let Some(size) = header.rsplit(' ').next().and_then(|s| s.trim().parse::<usize>().ok())
            else {
                break;
            };
            let Some(content) = bytes.get(at..at + size) else {
                break;
            };
            out.insert(rev.clone(), String::from_utf8_lossy(content).into_owned());
            at += size + 1;
        }
        Ok(out)
    }

    /// One file out of a revision, without materialising the tree.
    ///
    /// `files_at` is two processes and reads the whole tree; a caller that
    /// wants one file pays for 91 it will discard. One `git show` is cheaper
    /// when the answer really is a single file at a single revision. For the
    /// same file across several revisions use `file_across`, which is one
    /// process for all of them.
    pub fn file_at(&self, rev: &str, path: &str) -> Result<String> {
        let spec = format!("{rev}:{path}");
        Ok(git::run_ok(&self.path, &["show", &spec])?.stdout)
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
        self.resolve_within(pin, &self.tags()?)
    }

    /// `resolve` for a caller that has already listed the tags.
    ///
    /// `protocol.get` needs the list anyway — it puts it in the payload — and
    /// then `resolve` listed them again, so opening the protocol screen ran
    /// `git tag --list` twice for one answer.
    pub fn resolve_within(
        &self,
        pin: Option<&str>,
        tags: &[String],
    ) -> Result<(ProtocolRef, ProtocolFiles)> {
        let tag = match pin {
            Some(p) if !p.trim().is_empty() => {
                let p = p.trim().to_string();
                if !tags.contains(&p) {
                    return Err(err(format!(
                        "项目固定了协议版本 `{p}`，但本机的协议仓库里没有这个标签"
                    )));
                }
                p
            }
            _ => tags
                .last()
                .cloned()
                .ok_or_else(|| err("协议仓库里没有任何 protocol/vN 标签"))?,
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
        self.offer_upstream_of(seed(), &format!("{TAG_PREFIX}{SEED_VERSION}-upstream"))
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

    /// `file_at` is the whole-tree read narrowed to one path; it has to agree
    /// with it, or the version page would quietly show a different CHANGELOG
    /// than the one in the tag.
    /// The batched read has to agree with the single read, and has to skip a
    /// revision that lacks the file rather than shifting every later answer by
    /// one — `--batch` reports those as `<name> missing`, with no body.
    #[test]
    fn the_same_file_across_revisions_matches_reading_them_one_at_a_time() {
        if !git_available() {
            return;
        }
        let home = Home::new("file-across");
        let repo = ensure(&home.0).unwrap();
        let v1 = repo.release("v1").unwrap();
        std::fs::write(repo.path.join("CHANGELOG.md"), "# 第二版\n").unwrap();
        let v2 = repo.release("v2").unwrap();

        let tags = vec![v1.tag.clone(), v2.tag.clone()];
        let batched = repo.file_across(&tags, "CHANGELOG.md").unwrap();
        assert_eq!(batched.len(), 2);
        for tag in &tags {
            assert_eq!(batched[tag], repo.file_at(tag, "CHANGELOG.md").unwrap(), "{tag}");
        }
        assert_eq!(batched[&v2.tag], "# 第二版\n");

        // A file only the later version has: the earlier tag is skipped and
        // the later one still lands on its own key.
        std::fs::write(repo.path.join("NEW.md"), "只在 v3\n").unwrap();
        let v3 = repo.release("v3").unwrap();
        let mixed = repo
            .file_across(&[v1.tag.clone(), v3.tag.clone()], "NEW.md")
            .unwrap();
        assert_eq!(mixed.keys().collect::<Vec<_>>(), vec![&v3.tag]);
        assert_eq!(mixed[&v3.tag], "只在 v3\n");
    }

    #[test]
    fn one_file_out_of_a_revision_matches_the_whole_tree_read() {
        if !git_available() {
            return;
        }
        let home = Home::new("file-at");
        let repo = ensure(&home.0).unwrap();
        let v1 = repo.release("v1").unwrap();

        let whole = repo.files_at(&v1.tag).unwrap();
        for path in ["CHANGELOG.md", LOOP_PROTOCOL] {
            assert_eq!(
                repo.file_at(&v1.tag, path).unwrap(),
                *whole.get(path).expect("seed ships this file"),
                "{path}"
            );
        }
        // A path the revision does not have is an error, not an empty string:
        // silently returning "" would parse as a changelog with no entries.
        assert!(repo.file_at(&v1.tag, "nope.md").is_err());
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
        let mut next_seed = seed().clone();
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
