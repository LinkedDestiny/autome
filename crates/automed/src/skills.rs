//! Filesystem scanner for the skill inventory. Technical design §12,
//! requirement S-01.
//!
//! Autome does not own skills and never writes into a skill directory. It only
//! needs to answer one question for the binding UI and for `config::validate`:
//! *which skills exist, and which runtime can see each one*. Design §3.1 fixes
//! the layout — `.claude/skills` is what Claude Code reads, `.agents/skills` is
//! what Codex reads — and §12 fixes the rule: any immediate subdirectory of one
//! of those roots that contains a `SKILL.md` is a skill.
//!
//! The domain crate owns the shape of the answer (`SkillInventory`, and the
//! four roots via `scan_roots`); this module owns the only part that needs a
//! filesystem. Keeping the split here means the merge rules stay unit-testable
//! without touching a disk, and the disk-touching part stays small enough to
//! reason about.
//!
//! Three properties matter more than completeness, because the scan runs on
//! every project switch and its result gates a save:
//!
//! 1. **It never fails.** A root that does not exist, a root the user cannot
//!    read, a dangling symlink — all of these mean "no skills here", not an
//!    error. A permission problem on one root must not hide the other three.
//!    There is no useful way for the UI to recover from a partial failure, and
//!    reporting one would only teach the user to ignore it.
//! 2. **It never recurses.** Only one level down. A `SKILL.md` nested two
//!    levels deep belongs to some other tool's convention, not ours, and
//!    walking a whole home directory to find it would be both slow and wrong.
//! 3. **It is deterministic.** `read_dir` yields entries in whatever order the
//!    filesystem feels like; `SkillInventory::from_sources` sorts both the
//!    names and the sources within a name, so two scans of the same tree are
//!    byte-identical. The UI can therefore diff inventories to decide whether
//!    to emit a `skills.changed` event (§14).

use std::fs;

use autome_domain::role::Runtime;
use autome_domain::skill::{Scope, SkillInventory, SkillSource, scan_roots};

/// The marker file that makes a directory a skill (design §12).
const SKILL_MARKER: &str = "SKILL.md";

/// Scans the four roots for one project and returns the merged inventory.
///
/// `home` is the user's home directory and `repo` the repository root; both are
/// absolute. Same-named skills found under several roots merge into a single
/// `Skill` carrying every source, which is what makes a skill symlinked into
/// both runtime directories show up as one entry visible to both runtimes
/// rather than as two unrelated skills.
pub fn scan(home: &str, repo: &str) -> SkillInventory {
    SkillInventory::from_sources(collect(scan_roots(home, repo)))
}

/// Scans only the two global roots, for when no project is selected.
///
/// The roots still come from `scan_roots` rather than being rebuilt here: the
/// global layout is the domain crate's fact, and deriving it twice is how the
/// two copies eventually disagree. The repository argument is empty because
/// every path it would produce is discarded by the scope filter below.
pub fn scan_global(home: &str) -> SkillInventory {
    let roots = scan_roots(home, "")
        .into_iter()
        .filter(|(_, scope, _)| *scope == Scope::Global);
    SkillInventory::from_sources(collect(roots))
}

/// Walks every root and flattens the result into the `(name, source)` pairs
/// `SkillInventory::from_sources` expects.
fn collect(
    roots: impl IntoIterator<Item = (Runtime, Scope, String)>,
) -> Vec<(String, SkillSource)> {
    let mut found = Vec::new();
    for (runtime, scope, root) in roots {
        scan_root(runtime, scope, &root, &mut found);
    }
    found
}

/// Scans one root directory, appending whatever it finds.
///
/// Every failure path is a `continue` or an early return. That is deliberate:
/// see property 1 in the module docs.
fn scan_root(runtime: Runtime, scope: Scope, root: &str, found: &mut Vec<(String, SkillSource)>) {
    // A root that is absent, is a file, or cannot be read is simply empty.
    // Distinguishing those cases would give the caller nothing to act on.
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries {
        // An entry can fail on its own — a file removed mid-scan, for
        // instance. Skip it; the rest of the directory is still good.
        let Ok(entry) = entry else {
            continue;
        };

        let raw_name = entry.file_name();
        // A non-UTF-8 name cannot round-trip through the config file or the
        // JSON protocol, so such a skill could never be bound even if we
        // listed it. Better to not list it at all than to list something the
        // user cannot select.
        let Some(name) = raw_name.to_str() else {
            continue;
        };
        // Dot-directories are the runtimes' own bookkeeping (`.git`,
        // `.DS_Store`, editor scratch), never skills.
        if name.starts_with('.') {
            continue;
        }

        // Built from two known-UTF-8 pieces rather than from `PathBuf`, so the
        // recorded path is exact rather than lossily converted, and so it
        // matches the separator style `scan_roots` already produced.
        let path = format!("{}/{}", root.trim_end_matches('/'), name);

        if !is_dir(&path) || !is_file(&format!("{path}/{SKILL_MARKER}")) {
            continue;
        }

        found.push((
            name.to_string(),
            SkillSource {
                runtime,
                scope,
                path,
            },
        ));
    }
}

/// True when `path` resolves to a directory, following symlinks.
///
/// Symlinked skill directories are followed on purpose. Sharing one skill
/// between both runtimes by symlinking it into `.claude/skills` and
/// `.agents/skills` is the documented way to make it visible to both, and
/// refusing to follow the link would make that arrangement invisible.
///
/// Following is safe here only because nothing below recurses: we resolve the
/// entry itself and then look for exactly one file inside it, so a symlink loop
/// costs a single `ELOOP` from the kernel — reported as `Err`, and treated as
/// "not a directory" — rather than an unbounded walk.
fn is_dir(path: &str) -> bool {
    fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

/// True when `path` resolves to a regular file, following symlinks.
///
/// The marker must be a file: a *directory* named `SKILL.md` is not a skill
/// definition, and treating it as one would surface a broken skill that neither
/// runtime can actually load.
fn is_file(path: &str) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A throwaway directory tree, removed when the test ends.
    ///
    /// The scanner's whole job is to interpret real filesystem shapes —
    /// symlinks, missing roots, a directory where a file was expected — so the
    /// tests build real trees instead of mocking a filesystem trait. A mock
    /// would only prove that the mock agrees with our assumptions, which is
    /// exactly the thing in doubt.
    ///
    /// Cleanup lives in `Drop` rather than at the end of each test so that a
    /// failing assertion still leaves no litter in the system temp directory.
    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        /// Unique per process *and* per call, so tests running concurrently in
        /// the same binary — which is the default — cannot collide.
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "autome-skills-{}-{}-{}",
                std::process::id(),
                label,
                n
            ));
            // A leftover from a previous crashed run would make the test lie.
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("create temp root");
            Self { root }
        }

        fn path(&self, rel: &str) -> PathBuf {
            self.root.join(rel)
        }

        fn str(&self, rel: &str) -> String {
            self.path(rel)
                .to_str()
                .expect("utf-8 temp path")
                .to_string()
        }

        fn home(&self) -> String {
            self.str("home")
        }

        fn repo(&self) -> String {
            self.str("repo")
        }

        fn mkdir(&self, rel: &str) -> PathBuf {
            let p = self.path(rel);
            fs::create_dir_all(&p).expect("create dir");
            p
        }

        fn write(&self, rel: &str, contents: &str) {
            let p = self.path(rel);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            fs::write(&p, contents).expect("write file");
        }

        /// Creates `<rel>/SKILL.md`, i.e. a directory the scanner must accept.
        fn skill(&self, rel: &str) {
            self.write(&format!("{rel}/{SKILL_MARKER}"), "# skill\n");
        }

        #[cfg(unix)]
        fn symlink(&self, target: &Path, rel: &str) {
            let link = self.path(rel);
            if let Some(parent) = link.parent() {
                fs::create_dir_all(parent).expect("create parent");
            }
            std::os::unix::fs::symlink(target, &link).expect("symlink");
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            // Best effort: a cleanup failure must not turn a passing test red.
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn names(inv: &SkillInventory) -> Vec<String> {
        inv.iter().map(|s| s.name.clone()).collect()
    }

    #[test]
    fn a_skill_under_the_claude_home_directory_is_visible_to_claude_only() {
        let t = TempTree::new("claude-global");
        t.skill("home/.claude/skills/repo-facts");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(names(&inv), vec!["repo-facts"]);
        let skill = inv.get("repo-facts").unwrap();
        assert_eq!(skill.visible_to(), vec![Runtime::Claude]);
        assert!(!skill.is_project_scoped());
    }

    #[test]
    fn a_skill_under_the_codex_home_directory_is_visible_to_codex_only() {
        let t = TempTree::new("codex-global");
        t.skill("home/.agents/skills/repo-facts");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(
            inv.get("repo-facts").unwrap().visible_to(),
            vec![Runtime::Codex]
        );
    }

    #[test]
    fn the_same_name_under_both_runtime_directories_merges_into_one_skill() {
        // The §12 rule with teeth: one skill, two runtimes — not two skills.
        let t = TempTree::new("both-runtimes");
        t.skill("repo/.claude/skills/conventions");
        t.skill("repo/.agents/skills/conventions");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(inv.len(), 1);
        let skill = inv.get("conventions").unwrap();
        assert_eq!(skill.visible_to(), vec![Runtime::Claude, Runtime::Codex]);
        assert_eq!(skill.sources.len(), 2);
    }

    #[test]
    fn a_directory_without_a_skill_marker_is_not_a_skill() {
        let t = TempTree::new("no-marker");
        t.mkdir("home/.claude/skills/not-a-skill");
        t.write("home/.claude/skills/not-a-skill/README.md", "hi");
        t.skill("home/.claude/skills/real");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(names(&inv), vec!["real"]);
    }

    #[test]
    fn a_marker_that_is_a_directory_rather_than_a_file_is_not_a_skill() {
        // Neither runtime can load a directory as a skill definition, so
        // listing it would only offer the user an unbindable entry.
        let t = TempTree::new("marker-is-dir");
        t.mkdir(&format!("home/.claude/skills/odd/{SKILL_MARKER}"));

        assert!(scan(&t.home(), &t.repo()).is_empty());
    }

    #[test]
    fn a_plain_file_sitting_in_a_root_is_ignored() {
        let t = TempTree::new("file-in-root");
        t.mkdir("home/.claude/skills");
        t.write("home/.claude/skills/README.md", "not a skill");
        t.skill("home/.claude/skills/real");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(names(&inv), vec!["real"]);
    }

    #[test]
    fn a_dot_directory_is_ignored_even_with_a_marker() {
        let t = TempTree::new("dot-dir");
        t.skill("home/.claude/skills/.hidden");
        t.skill("home/.claude/skills/visible");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(names(&inv), vec!["visible"]);
    }

    #[test]
    fn a_marker_two_levels_down_is_not_picked_up() {
        // Property 2: one level only. `skills/pack/sub/SKILL.md` belongs to
        // some other tool's layout, and finding it would mean walking trees we
        // have no business walking.
        let t = TempTree::new("nested");
        t.write(
            &format!("home/.claude/skills/pack/sub/{SKILL_MARKER}"),
            "# nested\n",
        );

        assert!(scan(&t.home(), &t.repo()).is_empty());
    }

    #[test]
    fn a_root_that_does_not_exist_yields_nothing_and_does_not_error() {
        // The common case on a fresh machine: three of the four roots are
        // missing and the scan must still return the fourth.
        let t = TempTree::new("missing-roots");
        t.skill("home/.claude/skills/only");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(names(&inv), vec!["only"]);
    }

    #[test]
    fn scanning_a_tree_with_no_roots_at_all_is_empty() {
        let t = TempTree::new("bare");

        assert!(scan(&t.home(), &t.repo()).is_empty());
        assert!(scan_global(&t.home()).is_empty());
    }

    #[test]
    fn an_empty_root_directory_yields_nothing() {
        let t = TempTree::new("empty-root");
        t.mkdir("home/.claude/skills");

        assert!(scan(&t.home(), &t.repo()).is_empty());
    }

    #[test]
    fn a_project_skill_and_a_global_skill_of_the_same_name_merge_as_project_scoped() {
        // Requirement S-02: a skill that travels with the repository must be
        // reported as project-scoped even when a global one shadows the name.
        let t = TempTree::new("scopes");
        t.skill("home/.claude/skills/shared");
        t.skill("repo/.claude/skills/shared");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(inv.len(), 1);
        let skill = inv.get("shared").unwrap();
        assert!(skill.is_project_scoped());
        assert_eq!(skill.sources.len(), 2);
        assert_eq!(skill.visible_to(), vec![Runtime::Claude]);
    }

    #[test]
    fn all_four_roots_are_scanned() {
        let t = TempTree::new("four-roots");
        t.skill("home/.claude/skills/a");
        t.skill("home/.agents/skills/b");
        t.skill("repo/.claude/skills/c");
        t.skill("repo/.agents/skills/d");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(names(&inv), vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn scan_global_does_not_look_at_the_repository() {
        // Used when no project is selected, so a repository skill leaking in
        // would let the UI offer a binding that does not exist yet.
        let t = TempTree::new("global-only");
        t.skill("home/.claude/skills/global-one");
        t.skill("repo/.claude/skills/project-one");

        let inv = scan_global(&t.home());

        assert_eq!(names(&inv), vec!["global-one"]);
        assert!(!inv.get("global-one").unwrap().is_project_scoped());
    }

    #[test]
    fn the_recorded_path_points_at_the_skill_directory_itself() {
        let t = TempTree::new("paths");
        t.skill("home/.agents/skills/pathy");

        let inv = scan(&t.home(), &t.repo());

        let source = &inv.get("pathy").unwrap().sources[0];
        assert_eq!(source.path, t.str("home/.agents/skills/pathy"));
        assert_eq!(source.scope, Scope::Global);
        assert_eq!(source.runtime, Runtime::Codex);
    }

    #[test]
    fn trailing_slashes_on_home_and_repo_do_not_corrupt_the_recorded_paths() {
        let t = TempTree::new("trailing-slash");
        t.skill("home/.claude/skills/tidy");

        let inv = scan(&format!("{}/", t.home()), &format!("{}/", t.repo()));

        assert_eq!(names(&inv), vec!["tidy"]);
        assert!(!inv.get("tidy").unwrap().sources[0].path.contains("//"));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_skill_directory_is_followed() {
        // The documented way to share one skill between both runtimes: keep it
        // in one place, symlink it into the other runtime's directory.
        let t = TempTree::new("symlink");
        t.skill("shared/toolbelt");
        let target = t.path("shared/toolbelt");
        t.mkdir("home/.claude/skills");
        t.mkdir("home/.agents/skills");
        t.symlink(&target, "home/.claude/skills/toolbelt");
        t.symlink(&target, "home/.agents/skills/toolbelt");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(inv.len(), 1);
        assert_eq!(
            inv.get("toolbelt").unwrap().visible_to(),
            vec![Runtime::Claude, Runtime::Codex]
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_is_ignored() {
        let t = TempTree::new("dangling");
        t.mkdir("home/.claude/skills");
        t.symlink(
            Path::new("/nonexistent/target/xyz"),
            "home/.claude/skills/gone",
        );
        t.skill("home/.claude/skills/real");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(names(&inv), vec!["real"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_loop_terminates_and_is_ignored() {
        // `skills/loop` points back at `skills`, so a recursive walker would
        // spin forever. This scanner resolves one level and asks for one file,
        // so the kernel's ELOOP is the worst that can happen.
        let t = TempTree::new("loop");
        let root = t.mkdir("home/.claude/skills");
        t.symlink(&root, "home/.claude/skills/loop");
        t.skill("home/.claude/skills/real");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(names(&inv), vec!["real"]);
    }

    #[test]
    fn scanning_the_same_tree_twice_gives_identical_results() {
        // Property 3. `read_dir` order is not specified, so the inventory must
        // be normalised before it reaches the UI; otherwise `skills.changed`
        // (§14) would fire on noise.
        let t = TempTree::new("deterministic");
        for name in ["zeta", "alpha", "mid", "beta"] {
            t.skill(&format!("home/.claude/skills/{name}"));
            t.skill(&format!("home/.agents/skills/{name}"));
            t.skill(&format!("repo/.claude/skills/{name}"));
        }

        let first = scan(&t.home(), &t.repo());
        let second = scan(&t.home(), &t.repo());

        assert_eq!(first, second);
        assert_eq!(names(&first), vec!["alpha", "beta", "mid", "zeta"]);
        assert_eq!(first.get("alpha").unwrap().sources.len(), 3);
    }

    #[test]
    fn a_root_that_is_a_file_instead_of_a_directory_is_ignored() {
        // Not hypothetical: `.claude` can be a file in a repository that uses
        // the name for something else. It must not abort the other roots.
        let t = TempTree::new("root-is-file");
        t.write("repo/.claude/skills", "oops");
        t.skill("home/.claude/skills/survivor");

        let inv = scan(&t.home(), &t.repo());

        assert_eq!(names(&inv), vec!["survivor"]);
    }

    #[test]
    fn an_unreadable_root_does_not_abort_the_other_roots() {
        // Property 1. Running as root would defeat the permission bit, so the
        // assertion is written to hold either way: the readable root is found
        // regardless, and that is the behaviour under test.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let t = TempTree::new("unreadable");
            t.skill("repo/.claude/skills/locked");
            t.skill("home/.claude/skills/reachable");
            let locked_root = t.path("repo/.claude/skills");
            fs::set_permissions(&locked_root, fs::Permissions::from_mode(0o000))
                .expect("chmod 000");

            let inv = scan(&t.home(), &t.repo());

            // Restore before `Drop` tries to remove the tree.
            let _ = fs::set_permissions(&locked_root, fs::Permissions::from_mode(0o755));

            assert!(inv.get("reachable").is_some(), "one bad root hid another");
        }
    }
}
