//! The protocol left the binary; the prompts did not change.
//!
//! Moving four hundred lines of Chinese prose out of two `&'static str`
//! constants and into template files is exactly the kind of change that loses
//! a sentence, a blank line or a trailing space without anyone noticing —
//! and every one of those sentences is a rule some round previously invented
//! wrongly in a real run.
//!
//! `tests/golden/prompts/` holds what the constants rendered on 2026-09-17,
//! immediately before the move, with `{slug}`, `{request}` and `{budget_line}`
//! put back where the core substitutes them. These tests render the same
//! prompts from the protocol seed and compare byte for byte.
//!
//! When a protocol version deliberately changes a prompt, this goes red. That
//! is the point: read the diff, then update the golden file in the same
//! commit. See `tests/golden/README.md`.

use autome_domain::protocol::ProtocolFiles;
use autome_domain::role::Role;
use autome_domain::session::SessionKind;
use automed::launcher::{PromptSpec, build_prompt};

/// A sentinel no prompt contains, so the substitution can be reversed.
const SLUG: &str = "\u{1}";
const REQUEST: &str = "\u{2}";

fn golden(name: &str) -> String {
    let path = format!("{}/tests/golden/prompts/{name}.md", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn spec<'a>(kind: SessionKind, templates: &'a ProtocolFiles) -> PromptSpec<'a> {
    PromptSpec {
        kind,
        templates,
        slug: SLUG,
        design_rounds: 15,
        task_metrics: None,
        budget: None,
        request: REQUEST,
        skills: &[],
        inject: None,
        decisions: &[],
        attachments: &[],
        doc_refs: &[],
    }
}

fn rendered(kind: SessionKind) -> String {
    let templates = automed::protocol::seed();
    build_prompt(&spec(kind, &templates))
        .unwrap()
        .replace(SLUG, "{slug}")
        .replace(REQUEST, "{request}")
}

fn diff_report(name: &str, got: &str, want: &str) -> String {
    let g: Vec<&str> = got.lines().collect();
    let w: Vec<&str> = want.lines().collect();
    for (i, (a, b)) in g.iter().zip(w.iter()).enumerate() {
        if a != b {
            return format!(
                "{name} 第 {} 行不一致：\n  现在：{a}\n  基准：{b}",
                i + 1
            );
        }
    }
    format!(
        "{name} 行数不一致：现在 {} 行，基准 {} 行",
        g.len(),
        w.len()
    )
}

#[test]
fn every_role_prompt_renders_exactly_what_the_constants_did() {
    for role in [
        Role::Plan,
        Role::Review,
        Role::Adjudicate,
        Role::Impl,
        Role::Audit,
    ] {
        let name = role.as_str();
        // The golden files were captured with no budget, so the budget line
        // renders empty — which is also the case for three of these roles in
        // production, and the placeholder's position is what is being checked.
        let got = rendered(SessionKind::Role { role }).replace("{budget_line}", "");
        let want = golden(name).replace("{budget_line}", "");
        assert_eq!(got, want, "{}", diff_report(name, &got, &want));
    }
}

#[test]
fn the_budget_placeholder_sits_where_the_core_used_to_write_the_budget_line() {
    // Not just "the text is the same with the line removed": the line has to
    // go back in the same place, which is right after the opening sentence and
    // before the round's own instructions.
    for role in [Role::Impl, Role::Audit] {
        let want = golden(role.as_str());
        let (head, tail) = want
            .split_once("{budget_line}")
            .unwrap_or_else(|| panic!("{} 的基准里没有 {{budget_line}}", role.as_str()));

        let templates = automed::protocol::seed();
        let mut s = spec(SessionKind::Role { role }, &templates);
        s.budget = Some(automed::launcher::BudgetLine {
            round: 7,
            limit: 35,
        });
        let got = build_prompt(&s).unwrap().replace(SLUG, "{slug}");

        assert!(got.starts_with(head), "{} 的开头变了", role.as_str());
        assert!(got.ends_with(tail), "{} 的结尾变了", role.as_str());
        let middle = &got[head.len()..got.len() - tail.len()];
        assert!(middle.contains("N = 35"), "预算行没有落在占位符的位置");
    }
}

#[test]
fn the_intake_prompt_renders_exactly_what_the_constant_did() {
    let got = rendered(SessionKind::Intake);
    let want = golden("intake");
    assert_eq!(got, want, "{}", diff_report("intake", &got, &want));
}

#[test]
fn the_onboarding_prompt_renders_exactly_what_the_constant_did() {
    let got = rendered(SessionKind::Onboarding);
    let want = golden("onboarding");
    assert_eq!(got, want, "{}", diff_report("onboarding", &got, &want));
}

#[test]
fn a_project_that_lowered_its_design_round_limit_gets_that_limit_in_the_skeleton() {
    // Before the move this was the literal `15` regardless of configuration,
    // so a project with `design_rounds = 8` got an intake prompt telling it to
    // write `0/15`.
    let templates = automed::protocol::seed();
    let mut s = spec(SessionKind::Intake, &templates);
    s.design_rounds = 8;
    let p = build_prompt(&s).unwrap();
    assert!(p.contains("design-round: 0/8"), "{p}");
    assert!(!p.contains("design-round: 0/15"), "{p}");
}

#[test]
fn the_retro_prompt_is_handed_the_tasks_numbers() {
    use autome_domain::metrics::TaskMetrics;
    let templates = automed::protocol::seed();
    let metrics = TaskMetrics {
        impl_rounds_used: 15,
        budget_n: 35,
        reopen_total: 6,
        impl_defects: 8,
        reopen_by_domain: vec![("promo-case".into(), 2)],
        ..Default::default()
    };
    let mut s = spec(
        SessionKind::Role {
            role: Role::Retro,
        },
        &templates,
    );
    s.task_metrics = Some(&metrics);
    let p = build_prompt(&s).unwrap();
    assert!(p.contains("15/35"), "{p}");
    assert!(p.contains("promo-case × 2"), "{p}");
    assert!(p.contains("| 实现缺陷 | 8 |"), "{p}");
}

#[test]
fn a_task_with_no_recorded_metrics_is_told_so_rather_than_shown_zeroes() {
    // A retro round handed a table of zeroes would write about a task that
    // went perfectly. Absence has to read as absence.
    let templates = automed::protocol::seed();
    let p = build_prompt(&spec(
        SessionKind::Role {
            role: Role::Retro,
        },
        &templates,
    ))
    .unwrap();
    assert!(p.contains("没有记录到指标"), "{p}");
    assert!(!p.contains("| 实现缺陷 | 0 |"), "{p}");
}

#[test]
fn a_template_with_an_unknown_placeholder_is_refused_rather_than_shown_to_the_model() {
    let mut templates = automed::protocol::seed();
    let text = format!("{}\n还要参考 {{mood}}。\n", templates.prompt("impl").unwrap());
    templates.insert("prompts/impl.md", text);
    let e = build_prompt(&spec(
        SessionKind::Role { role: Role::Impl },
        &templates,
    ))
    .unwrap_err();
    assert!(e.detail.contains("mood"), "{}", e.detail);
}

#[test]
fn braces_in_ordinary_prose_are_not_mistaken_for_placeholders() {
    // The protocol text contains code samples and JSON. A check that rejected
    // those would be unusable.
    let mut templates = automed::protocol::seed();
    let text = format!(
        "{}\n示例：`{{ \"status\": \"ok\" }}`，以及 {{M-01}}。\n",
        templates.prompt("impl").unwrap()
    );
    templates.insert("prompts/impl.md", text);
    assert!(
        build_prompt(&spec(
            SessionKind::Role { role: Role::Impl },
            &templates
        ))
        .is_ok()
    );
}

#[test]
fn a_version_missing_a_role_template_names_the_role_rather_than_launching_blank() {
    let mut templates = automed::protocol::seed();
    templates.remove("prompts/audit.md");
    let e = build_prompt(&spec(
        SessionKind::Role { role: Role::Audit },
        &templates,
    ))
    .unwrap_err();
    assert!(e.detail.contains("prompts/audit.md"), "{}", e.detail);
    assert!(e.detail.contains("审计轮"), "{}", e.detail);
}
