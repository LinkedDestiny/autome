//! An eval case: what a protocol version is supposed to make a round *do*.
//!
//! Layers 1 and 2 check the text. This is the layer that checks the behaviour,
//! and it is the only one that costs money — so the shape of a case is chosen
//! to keep the bill bounded and the signal high:
//!
//! - **The fixture is a task directory and nothing else.** No project code, no
//!   build. A case asserts the first few steps of a round — which files it
//!   opened, what it wrote, what its first command was — not that it finished
//!   a task.
//! - **`max_turns` caps how much work a case may need.** Neither CLI offers a
//!   flag for it (checked against Claude Code 2.1.261 and Codex 0.153.4 on
//!   2026-09-17), so it is enforced two ways: a wall-clock timeout stops a run
//!   that will not end, and the turn count read back out of the stream fails a
//!   run that needed more turns than the case allows. The second is the one
//!   that matters — a case that suddenly needs twenty turns is no longer
//!   asserting what it was written to assert.
//! - **Every grader carries a positive control.** A grader that cannot fail is
//!   a grader that says nothing, and the ones that only ever pass are exactly
//!   the ones nobody notices.

use autome_domain::role::Role;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    pub name: String,
    pub role: Role,
    pub description: String,
    /// How many times to run it. The verdict is the majority.
    pub runs: u32,
    pub max_turns: u32,
    pub timeout_seconds: u64,
    /// Path to the scaffold script, relative to the case directory.
    pub scaffold: String,
    /// Grader files, relative to the case directory.
    pub graders: Vec<String>,
}

pub const DEFAULT_RUNS: u32 = 3;
pub const DEFAULT_MAX_TURNS: u32 = 15;
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 900;

/// Parses `case.yaml`.
///
/// The same restricted reader the rest of the system uses, for the same
/// reason: the schema is eight closed fields, and an unknown key has to be an
/// error naming it rather than a silently ignored line. A typo'd `max_turns`
/// would give a case an unbounded budget without anyone noticing.
pub fn parse(text: &str) -> Result<Case, String> {
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut graders: Vec<String> = Vec::new();
    let mut in_graders = false;

    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim_start().starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some(item) = line.trim_start().strip_prefix("- ") {
            if !in_graders {
                return Err(format!("`{}` 不在任何列表下面", item.trim()));
            }
            graders.push(item.trim().to_string());
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(format!("`{line}` 不是 `字段: 值`"));
        };
        let key = key.trim().to_string();
        let value = value.trim().to_string();
        in_graders = key == "graders";
        if in_graders && value.is_empty() {
            continue;
        }
        fields.push((key, value));
    }

    let get = |k: &str| fields.iter().find(|(f, _)| f == k).map(|(_, v)| v.as_str());
    let known = [
        "name",
        "role",
        "description",
        "runs",
        "max_turns",
        "timeout_seconds",
        "scaffold",
        "graders",
    ];
    if let Some((k, _)) = fields.iter().find(|(k, _)| !known.contains(&k.as_str())) {
        return Err(format!("case.yaml 里没有 `{k}` 这个字段"));
    }

    let num = |k: &str, default: u64| -> Result<u64, String> {
        match get(k) {
            None => Ok(default),
            Some(v) => v.parse().map_err(|_| format!("{k} `{v}` 不是整数")),
        }
    };

    let name = get("name").ok_or("case.yaml 缺少 name")?.to_string();
    let role_raw = get("role").ok_or("case.yaml 缺少 role")?;
    let role = Role::parse(role_raw).ok_or_else(|| {
        format!(
            "`{role_raw}` 不是角色，可选：{}",
            Role::ALL
                .iter()
                .map(|r| r.as_str())
                .collect::<Vec<_>>()
                .join(" / ")
        )
    })?;
    if graders.is_empty() {
        return Err(format!("{name} 一个 grader 都没有，跑了也不会得出结论"));
    }

    Ok(Case {
        name,
        role,
        description: get("description").unwrap_or("").to_string(),
        runs: num("runs", DEFAULT_RUNS as u64)? as u32,
        max_turns: num("max_turns", DEFAULT_MAX_TURNS as u64)? as u32,
        timeout_seconds: num("timeout_seconds", DEFAULT_TIMEOUT_SECONDS)?,
        scaffold: get("scaffold").unwrap_or("./scaffold.sh").to_string(),
        graders,
    })
}

/// Static checks over a case that do not need a model.
///
/// Run as part of layer 1, so a case that could never say anything is caught
/// before anyone pays to find out.
pub fn lint(case: &Case, grader_bodies: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    if case.runs == 0 {
        out.push(format!("{}: runs 是 0", case.name));
    }
    if case.runs.is_multiple_of(2) {
        out.push(format!(
            "{}: runs = {} 是偶数，取多数会平票",
            case.name, case.runs
        ));
    }
    if case.max_turns == 0 {
        out.push(format!("{}: max_turns 是 0", case.name));
    }
    for (path, body) in grader_bodies {
        // The one property that makes a grader worth running.
        if !body.contains("阳性对照") {
            out.push(format!(
                "{}: {path} 没有阳性对照。判不了负样本的 grader 等于没有——\
                 而永远只会通过的那些，恰恰是没人会注意到的。",
                case.name
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "# 一句话说明\n\
        name: impl-writes-evidence-not-design\n\
        role: impl\n\
        description: 证据进 evidence/\n\
        runs: 3\n\
        max_turns: 15\n\
        timeout_seconds: 900\n\
        scaffold: ./scaffold.sh\n\
        graders:\n\
        \x20 - graders/a.md\n\
        \x20 - graders/b.md\n";

    #[test]
    fn a_case_parses_every_field_and_its_grader_list() {
        let c = parse(YAML).unwrap();
        assert_eq!(c.name, "impl-writes-evidence-not-design");
        assert_eq!(c.role, Role::Impl);
        assert_eq!(c.runs, 3);
        assert_eq!(c.max_turns, 15);
        assert_eq!(c.timeout_seconds, 900);
        assert_eq!(c.graders, vec!["graders/a.md", "graders/b.md"]);
    }

    #[test]
    fn the_defaults_are_the_ones_the_plan_named() {
        let c = parse("name: x\nrole: audit\ngraders:\n  - g.md\n").unwrap();
        assert_eq!(c.runs, DEFAULT_RUNS);
        assert_eq!(c.max_turns, DEFAULT_MAX_TURNS);
        assert_eq!(c.timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
    }

    #[test]
    fn a_typo_in_a_field_name_is_an_error_rather_than_an_unbounded_budget() {
        let e = parse("name: x\nrole: impl\nmax_turn: 99\ngraders:\n  - g.md\n").unwrap_err();
        assert!(e.contains("max_turn"), "{e}");
    }

    #[test]
    fn an_unknown_role_names_the_ones_that_exist() {
        let e = parse("name: x\nrole: implementer\ngraders:\n  - g.md\n").unwrap_err();
        assert!(e.contains("impl"), "{e}");
    }

    #[test]
    fn a_case_with_no_graders_is_refused() {
        let e = parse("name: x\nrole: impl\n").unwrap_err();
        assert!(e.contains("grader"), "{e}");
    }

    #[test]
    fn an_even_number_of_runs_would_tie() {
        let c = parse("name: x\nrole: impl\nruns: 2\ngraders:\n  - g.md\n").unwrap();
        let problems = lint(&c, &[("g.md".into(), "阳性对照：…".into())]);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("平票"), "{problems:?}");
    }

    #[test]
    fn a_grader_without_a_positive_control_is_reported() {
        let c = parse(YAML).unwrap();
        let problems = lint(
            &c,
            &[
                ("graders/a.md".into(), "判定目标：…\n阳性对照：…".into()),
                ("graders/b.md".into(), "判定目标：…".into()),
            ],
        );
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("graders/b.md"), "{problems:?}");
    }

    #[test]
    fn every_seeded_case_parses_and_lints_clean() {
        // The cases ship in the binary; one that cannot be read would only
        // show up when someone paid to run it.
        let files = crate::protocol::seed();
        let mut cases = 0;
        for path in files.paths().collect::<Vec<_>>() {
            if !path.ends_with("/case.yaml") {
                continue;
            }
            cases += 1;
            let dir = path.trim_end_matches("case.yaml");
            let case = parse(files.get(path).unwrap()).unwrap_or_else(|e| panic!("{path}: {e}"));
            let bodies: Vec<(String, String)> = case
                .graders
                .iter()
                .map(|g| {
                    let full = format!("{dir}{g}");
                    let body = files
                        .get(&full)
                        .unwrap_or_else(|| panic!("{path} 引用了不存在的 {full}"));
                    (g.clone(), body.to_string())
                })
                .collect();
            let problems = lint(&case, &bodies);
            assert!(problems.is_empty(), "{path}: {problems:?}");
        }
        assert!(cases >= 9, "only {cases} cases are seeded");
    }
}
