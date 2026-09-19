//! Skill inventory. Technical design §12; requirements S-01 through S-06.
//!
//! 2.0 does not manage skills — it *reads* them. A skill is any directory
//! containing a `SKILL.md` under one of four roots, two per runtime (global in
//! `$HOME`, project-local in the repository). The only thing Autome owns is
//! the binding: which role must use which skill, stored in the project's
//! `.autome/config.toml`.
//!
//! The one rule with teeth is visibility: Claude Code cannot see a skill that
//! only exists under `.agents/skills`, so binding it to a Codex-running role
//! is refused (S-04). That check lives in `config::validate`; this module
//! supplies the inventory it consults.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::SkillVisibility;
use crate::role::Runtime;

/// Where a skill directory was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Under `$HOME` — available to every project.
    Global,
    /// Inside a repository — travels with the repo (requirement S-02's
    /// "Skill 随仓库走").
    Project,
}

/// One discovered skill directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSource {
    pub runtime: Runtime,
    pub scope: Scope,
    /// Absolute path to the skill directory.
    pub path: String,
}

/// A skill as the UI lists it: one entry per name, carrying every place it was
/// found. The same name under both runtimes' directories is one skill visible
/// to both, not two skills — that is the normal way a project ships a skill
/// for both CLIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub sources: Vec<SkillSource>,
}

impl Skill {
    /// Which runtimes can load this skill. Sorted and deduplicated so the
    /// order is stable for display and comparison.
    pub fn visible_to(&self) -> Vec<Runtime> {
        let mut rs: Vec<Runtime> = self.sources.iter().map(|s| s.runtime).collect();
        rs.sort_unstable();
        rs.dedup();
        rs
    }

    /// True when the skill exists anywhere inside a repository.
    pub fn is_project_scoped(&self) -> bool {
        self.sources.iter().any(|s| s.scope == Scope::Project)
    }
}

/// The whole inventory for one project: its own skills plus the global ones.
///
/// Serialize only, deliberately. The visibility cache below is derived from
/// `skills` and is not serialised, so a deserialised inventory would answer
/// every visibility question with "not visible" until something recomputed it
/// — a trap with no upside, since the only way one is ever built is by
/// scanning the filesystem.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SkillInventory {
    /// Keyed by name so lookup during validation is not a linear scan, and so
    /// the listing order is deterministic.
    pub skills: BTreeMap<String, Skill>,
    /// Cached `visible_to()` per skill, to satisfy `SkillVisibility`'s
    /// borrow-returning signature without recomputing on every call.
    #[serde(skip)]
    visibility: BTreeMap<String, Vec<Runtime>>,
}

impl SkillInventory {
    /// Builds an inventory from discovered directories, merging same-named
    /// entries across runtimes and scopes.
    pub fn from_sources(found: impl IntoIterator<Item = (String, SkillSource)>) -> Self {
        let mut skills: BTreeMap<String, Skill> = BTreeMap::new();
        for (name, source) in found {
            let entry = skills.entry(name.clone()).or_insert_with(|| Skill {
                name,
                sources: Vec::new(),
            });
            if !entry.sources.contains(&source) {
                entry.sources.push(source);
            }
        }
        for skill in skills.values_mut() {
            skill
                .sources
                .sort_by(|a, b| (a.scope, a.runtime, &a.path).cmp(&(b.scope, b.runtime, &b.path)));
        }
        let visibility = skills
            .iter()
            .map(|(n, s)| (n.clone(), s.visible_to()))
            .collect();
        Self { skills, visibility }
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Skill> {
        self.skills.values()
    }
}

impl SkillVisibility for SkillInventory {
    fn visible_to(&self, skill: &str) -> Option<&[Runtime]> {
        self.visibility.get(skill).map(Vec::as_slice)
    }
}

/// The four roots scanned for one project (design §12). Returned as
/// `(runtime, scope, relative-or-absolute path)` so `automed` can join them
/// against `$HOME` or the repository root without re-deriving the mapping.
pub fn scan_roots(home: &str, repo: &str) -> Vec<(Runtime, Scope, String)> {
    let mut roots = Vec::new();
    for runtime in Runtime::ALL {
        roots.push((
            runtime,
            Scope::Global,
            format!("{}/{}", home.trim_end_matches('/'), runtime.skill_dir()),
        ));
        roots.push((
            runtime,
            Scope::Project,
            format!("{}/{}", repo.trim_end_matches('/'), runtime.skill_dir()),
        ));
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(runtime: Runtime, scope: Scope, path: &str) -> SkillSource {
        SkillSource {
            runtime,
            scope,
            path: path.into(),
        }
    }

    #[test]
    fn a_skill_in_one_runtime_directory_is_visible_only_to_that_runtime() {
        let inv = SkillInventory::from_sources([(
            "repo-facts".to_string(),
            src(
                Runtime::Claude,
                Scope::Global,
                "/h/.claude/skills/repo-facts",
            ),
        )]);
        assert_eq!(
            inv.get("repo-facts").unwrap().visible_to(),
            vec![Runtime::Claude]
        );
        assert_eq!(
            SkillVisibility::visible_to(&inv, "repo-facts"),
            Some([Runtime::Claude].as_slice())
        );
    }

    #[test]
    fn the_same_name_under_both_runtimes_is_one_skill_visible_to_both() {
        let inv = SkillInventory::from_sources([
            (
                "conventions".to_string(),
                src(
                    Runtime::Claude,
                    Scope::Project,
                    "/r/.claude/skills/conventions",
                ),
            ),
            (
                "conventions".to_string(),
                src(
                    Runtime::Codex,
                    Scope::Project,
                    "/r/.agents/skills/conventions",
                ),
            ),
        ]);
        assert_eq!(inv.len(), 1);
        assert_eq!(
            inv.get("conventions").unwrap().visible_to(),
            vec![Runtime::Claude, Runtime::Codex]
        );
    }

    #[test]
    fn an_unknown_skill_has_no_visibility_entry() {
        let inv = SkillInventory::default();
        assert_eq!(SkillVisibility::visible_to(&inv, "ghost"), None);
    }

    #[test]
    fn duplicate_identical_sources_are_collapsed() {
        let s = src(Runtime::Claude, Scope::Global, "/h/.claude/skills/a");
        let inv =
            SkillInventory::from_sources([("a".to_string(), s.clone()), ("a".to_string(), s)]);
        assert_eq!(inv.get("a").unwrap().sources.len(), 1);
    }

    #[test]
    fn project_scope_is_reported_when_any_source_is_in_the_repo() {
        let inv = SkillInventory::from_sources([
            (
                "mixed".to_string(),
                src(Runtime::Claude, Scope::Global, "/h/.claude/skills/mixed"),
            ),
            (
                "mixed".to_string(),
                src(Runtime::Codex, Scope::Project, "/r/.agents/skills/mixed"),
            ),
        ]);
        assert!(inv.get("mixed").unwrap().is_project_scoped());
    }

    #[test]
    fn listing_order_is_alphabetical_and_stable() {
        let inv = SkillInventory::from_sources([
            (
                "zeta".to_string(),
                src(Runtime::Claude, Scope::Global, "/z"),
            ),
            (
                "alpha".to_string(),
                src(Runtime::Claude, Scope::Global, "/a"),
            ),
            ("mid".to_string(), src(Runtime::Claude, Scope::Global, "/m")),
        ]);
        let names: Vec<&str> = inv.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "mid", "zeta"]);
    }

    #[test]
    fn sources_within_a_skill_are_ordered_deterministically() {
        let build = |order: [SkillSource; 3]| {
            SkillInventory::from_sources(order.map(|s| ("x".to_string(), s)))
                .get("x")
                .unwrap()
                .sources
                .clone()
        };
        let a = src(Runtime::Claude, Scope::Global, "/h/.claude/skills/x");
        let b = src(Runtime::Codex, Scope::Global, "/h/.agents/skills/x");
        let c = src(Runtime::Claude, Scope::Project, "/r/.claude/skills/x");
        assert_eq!(
            build([a.clone(), b.clone(), c.clone()]),
            build([c, b, a]),
            "source order must not depend on discovery order"
        );
    }

    #[test]
    fn scan_roots_covers_four_directories_two_per_runtime() {
        let roots = scan_roots("/Users/x", "/Users/x/code/shop");
        assert_eq!(roots.len(), 4);
        let paths: Vec<&str> = roots.iter().map(|(_, _, p)| p.as_str()).collect();
        assert!(paths.contains(&"/Users/x/.claude/skills"));
        assert!(paths.contains(&"/Users/x/.agents/skills"));
        assert!(paths.contains(&"/Users/x/code/shop/.claude/skills"));
        assert!(paths.contains(&"/Users/x/code/shop/.agents/skills"));
    }

    #[test]
    fn scan_roots_tolerates_a_trailing_slash() {
        let roots = scan_roots("/Users/x/", "/Users/x/code/shop/");
        assert!(roots.iter().all(|(_, _, p)| !p.contains("//")));
    }

}
