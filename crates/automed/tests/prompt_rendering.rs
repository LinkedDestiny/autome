//! Nothing changes a prompt except a protocol version, deliberately.
//!
//! `tests/golden/prompts/` started as what the two compiled-in constants
//! rendered on 2026-09-17, immediately before the protocol left the binary —
//! moving four hundred lines of Chinese prose into template files is exactly
//! the kind of change that loses a sentence or a trailing space without anyone
//! noticing, and every one of those sentences is a rule some round previously
//! invented wrongly in a real run. It passed byte for byte.
//!
//! Since then the baseline moves when the protocol does. When a version
//! deliberately changes a prompt this goes red, and that is the point: read
//! the diff, then refresh the baseline in the same commit with
//! `AUTOME_UPDATE_GOLDEN=1 cargo test -p automed --test prompt_rendering`.
//! See `tests/golden/README.md`.

use autome_domain::protocol::ProtocolFiles;
use autome_domain::role::Role;
use autome_domain::session::SessionKind;
use automed::launcher::{PromptSpec, build_prompt};

/// A sentinel no prompt contains, so the substitution can be reversed.
const SLUG: &str = "\u{1}";
const REQUEST: &str = "\u{2}";
const BRIEF: &str = "\u{3}";
const DOC_DIR: &str = "\u{4}";

fn golden(name: &str) -> String {
    let path = format!(
        "{}/tests/golden/prompts/{name}.md",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Rewrites a baseline. Deliberately opt-in through the environment rather
/// than automatic: the value of this test is entirely in someone looking at
/// the diff before accepting it, and a baseline that updates itself is a test
/// that always passes.
fn maybe_update_golden(name: &str, got: &str) -> bool {
    if std::env::var("AUTOME_UPDATE_GOLDEN").is_err() {
        return false;
    }
    let path = format!(
        "{}/tests/golden/prompts/{name}.md",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::write(&path, got).unwrap();
    true
}

fn spec<'a>(kind: SessionKind, templates: &'a ProtocolFiles) -> PromptSpec<'a> {
    PromptSpec {
        kind,
        templates,
        gate_before_merge: None,
        brief_path: BRIEF,
        slug: SLUG,
        doc_dir: DOC_DIR,
        card_path: None,
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
    let templates = automed::protocol::seed().clone();
    build_prompt(&spec(kind, &templates))
        .unwrap()
        .replace(DOC_DIR, "{doc_dir}")
        .replace(SLUG, "{slug}")
        .replace(REQUEST, "{request}")
        .replace(BRIEF, "{brief_path}")
}

// T-62: the agent must be told the same write boundary that the scheduler
// enforces. Golden snapshots alone preserve the omission just as faithfully.
fn assert_document_only_scope(prompt: &str) {
    let plain = prompt.replace(['*', '`'], "");
    assert!(
        plain.contains("只能修改 {doc_dir}/ 下的文件"),
        "缺少本任务文档目录的明确写入边界"
    );
    for term in ["被跟踪", "产品代码", "测试", "格式化", "lint", "协议失败"] {
        assert!(plain.contains(term), "范围告知缺少：{term}");
    }
}

#[test]
fn scope_contract_audit_declares_guard_boundary() {
    assert_document_only_scope(&rendered(SessionKind::Role { role: Role::Audit }));
}

#[test]
fn scope_contract_retro_declares_guard_boundary() {
    assert_document_only_scope(&rendered(SessionKind::Role { role: Role::Retro }));
}

#[test]
fn scope_contract_fix_review_declares_guard_boundary() {
    assert_document_only_scope(&bug_rendered(
        autome_domain::session::BugStep::FixReview,
        false,
    ));
}

#[test]
fn scope_contract_audit_limits_verification_gap_repairs() {
    let prompt = rendered(SessionKind::Role { role: Role::Audit });
    let gap = prompt
        .lines()
        .find(|line| line.starts_with("- **验证缺口**"))
        .expect("审计必须保留验证缺口结论");
    // Check the instruction itself, not incidental mentions elsewhere in
    // the prompt: adding a warning must not leave this permission unbounded.
    for term in ["验收契约", ".autome/output/", "不进仓库", "实现轮"] {
        assert!(gap.contains(term), "验证缺口的操作范围缺少：{term}");
    }
}

fn diff_report(name: &str, got: &str, want: &str) -> String {
    let g: Vec<&str> = got.lines().collect();
    let w: Vec<&str> = want.lines().collect();
    for (i, (a, b)) in g.iter().zip(w.iter()).enumerate() {
        if a != b {
            return format!("{name} 第 {} 行不一致：\n  现在：{a}\n  基准：{b}", i + 1);
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
        // The baseline for a role is the template itself: the frame lives in
        // it too, so a rendered prompt with no budget differs from the
        // template by exactly the one placeholder.
        if maybe_update_golden(name, automed::protocol::seed().prompt(name).unwrap()) {
            continue;
        }
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

        let templates = automed::protocol::seed().clone();
        let mut s = spec(SessionKind::Role { role }, &templates);
        s.budget = Some(automed::launcher::BudgetLine {
            round: 7,
            limit: 35,
        });
        let got = build_prompt(&s)
            .unwrap()
            .replace(DOC_DIR, "{doc_dir}")
            .replace(SLUG, "{slug}")
            .replace(BRIEF, "{brief_path}");

        assert!(got.starts_with(head), "{} 的开头变了", role.as_str());
        assert!(got.ends_with(tail), "{} 的结尾变了", role.as_str());
        let middle = &got[head.len()..got.len() - tail.len()];
        assert!(middle.contains("N = 35"), "预算行没有落在占位符的位置");
    }
}

#[test]
fn the_intake_prompt_renders_exactly_what_the_constant_did() {
    let got = rendered(SessionKind::Intake);
    if maybe_update_golden("intake", &got) {
        return;
    }
    let want = golden("intake");
    assert_eq!(got, want, "{}", diff_report("intake", &got, &want));
}

#[test]
fn the_onboarding_prompt_renders_exactly_what_the_constant_did() {
    let got = rendered(SessionKind::Onboarding);
    if maybe_update_golden("onboarding", &got) {
        return;
    }
    let want = golden("onboarding");
    assert_eq!(got, want, "{}", diff_report("onboarding", &got, &want));
}

#[test]
fn a_project_that_lowered_its_design_round_limit_gets_that_limit_in_the_skeleton() {
    // Before the move this was the literal `15` regardless of configuration,
    // so a project with `design_rounds = 8` got an intake prompt telling it to
    // write `0/15`.
    let templates = automed::protocol::seed().clone();
    let mut s = spec(SessionKind::Intake, &templates);
    s.design_rounds = 8;
    let p = build_prompt(&s).unwrap();
    assert!(p.contains("design-round: 0/8"), "{p}");
    assert!(!p.contains("design-round: 0/15"), "{p}");
}

#[test]
fn the_retro_prompt_is_handed_the_tasks_numbers() {
    use autome_domain::metrics::TaskMetrics;
    let templates = automed::protocol::seed().clone();
    let metrics = TaskMetrics {
        impl_rounds_used: 15,
        budget_n: 35,
        reopen_total: 6,
        impl_defects: 8,
        reopen_by_domain: vec![("promo-case".into(), 2)],
        ..Default::default()
    };
    let mut s = spec(SessionKind::Role { role: Role::Retro }, &templates);
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
    let templates = automed::protocol::seed().clone();
    let p = build_prompt(&spec(SessionKind::Role { role: Role::Retro }, &templates)).unwrap();
    assert!(p.contains("没有记录到指标"), "{p}");
    assert!(!p.contains("| 实现缺陷 | 0 |"), "{p}");
}

#[test]
fn a_template_with_an_unknown_placeholder_is_refused_rather_than_shown_to_the_model() {
    let mut templates = automed::protocol::seed().clone();
    let text = format!(
        "{}\n还要参考 {{mood}}。\n",
        templates.prompt("impl").unwrap()
    );
    templates.insert("prompts/impl.md", text);
    let e = build_prompt(&spec(SessionKind::Role { role: Role::Impl }, &templates)).unwrap_err();
    assert!(e.detail.contains("mood"), "{}", e.detail);
}

#[test]
fn braces_in_ordinary_prose_are_not_mistaken_for_placeholders() {
    // The protocol text contains code samples and JSON. A check that rejected
    // those would be unusable.
    let mut templates = automed::protocol::seed().clone();
    let text = format!(
        "{}\n示例：`{{ \"status\": \"ok\" }}`，以及 {{M-01}}。\n",
        templates.prompt("impl").unwrap()
    );
    templates.insert("prompts/impl.md", text);
    assert!(build_prompt(&spec(SessionKind::Role { role: Role::Impl }, &templates)).is_ok());
}

#[test]
fn a_version_missing_a_role_template_names_the_role_rather_than_launching_blank() {
    let mut templates = automed::protocol::seed().clone();
    templates.remove("prompts/audit.md");
    let e = build_prompt(&spec(SessionKind::Role { role: Role::Audit }, &templates)).unwrap_err();
    assert!(e.detail.contains("prompts/audit.md"), "{}", e.detail);
    assert!(e.detail.contains("审计轮"), "{}", e.detail);
}

#[test]
fn the_intake_prompt_with_a_task_card_matches_its_golden() {
    let templates = automed::protocol::seed().clone();
    let mut s = spec(SessionKind::Intake, &templates);
    let path = format!("{DOC_DIR}/{SLUG}-card.md");
    s.card_path = Some(&path);
    let got = build_prompt(&s)
        .unwrap()
        .replace(DOC_DIR, "{doc_dir}")
        .replace(SLUG, "{slug}")
        .replace(REQUEST, "{request}")
        .replace(BRIEF, "{brief_path}");
    assert_eq!(got, golden("intake-card"));
    assert!(got.ends_with(&golden("intake")));
}

fn bug_rendered(step: autome_domain::session::BugStep, card: bool) -> String {
    let templates = automed::protocol::seed();
    let mut s = spec(SessionKind::Bug { step }, templates);
    s.budget = Some(automed::launcher::BudgetLine { round: 2, limit: 3 });
    let path = format!("{DOC_DIR}/{SLUG}-card.md");
    if card {
        s.card_path = Some(&path);
    }
    build_prompt(&s)
        .unwrap()
        .replace(DOC_DIR, "{doc_dir}")
        .replace(SLUG, "{slug}")
        .replace(REQUEST, "{request}")
}

fn check_bug_golden(name: &str, step: autome_domain::session::BugStep, card: bool) {
    let got = bug_rendered(step, card);
    if maybe_update_golden(name, &got) {
        return;
    }
    assert_eq!(got, golden(name));
}

#[test]
fn bug_loop_locate_prompt_matches_its_golden() {
    check_bug_golden("locate", autome_domain::session::BugStep::Locate, false);
}
#[test]
fn bug_loop_locate_prompt_with_a_card_matches_its_golden() {
    check_bug_golden("locate-card", autome_domain::session::BugStep::Locate, true);
    let got = bug_rendered(autome_domain::session::BugStep::Locate, true);
    assert!(got.ends_with(&golden("locate")));
    assert!(got.find("-card.md").unwrap() < got.find("{request}").unwrap());
}
#[test]
fn bug_loop_fix_prompt_matches_its_golden() {
    check_bug_golden("fix", autome_domain::session::BugStep::Fix, false);
}
#[test]
fn bug_loop_fix_review_prompt_matches_its_golden() {
    check_bug_golden(
        "fix-review",
        autome_domain::session::BugStep::FixReview,
        false,
    );
}
#[test]
fn bug_loop_bug_prompts_leave_no_placeholder() {
    use autome_domain::session::BugStep;
    for (step, name) in [
        (BugStep::Locate, "locate"),
        (BugStep::Fix, "fix"),
        (BugStep::FixReview, "fix-review"),
    ] {
        for (round, limit) in [(0, 1), (1, 1), (20, 20), (u32::MAX, u32::MAX)] {
            let mut templates = automed::protocol::seed().clone();
            let mut s = spec(SessionKind::Bug { step }, &templates);
            s.budget = Some(automed::launcher::BudgetLine { round, limit });
            s.request = "";
            let got = build_prompt(&s).unwrap();
            for placeholder in [
                "{doc_dir}",
                "{slug}",
                "{request}",
                "{budget_line}",
                "{brief_path}",
            ] {
                assert!(!got.contains(placeholder), "{got}");
            }
            let expected = if step == BugStep::Locate {
                format!("0/{limit}")
            } else {
                format!("{round}/{limit}")
            };
            assert!(got.contains(&expected));
            s.card_path = Some("成员/任务卡.md");
            s.request = "含 {literal} 的需求";
            if step == BugStep::Locate {
                assert!(build_prompt(&s).unwrap().contains(s.request));
            }
            s.budget = None;
            assert!(build_prompt(&s).unwrap_err().detail.contains("预算"));
            templates.remove(&format!("prompts/{name}.md"));
            let s = spec(SessionKind::Bug { step }, &templates);
            assert!(build_prompt(&s).unwrap_err().detail.contains(name));
        }
    }
}

fn brief_golden(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/golden/briefs/{name}.md",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn maybe_update_brief_golden(name: &str, got: &str) {
    if std::env::var("AUTOME_UPDATE_GOLDEN").as_deref() == Ok("1") {
        std::fs::write(
            format!(
                "{}/tests/golden/briefs/{name}.md",
                env!("CARGO_MANIFEST_DIR")
            ),
            got,
        )
        .unwrap();
    }
}

#[test]
fn before_merge_audit_brief_matches_its_golden() {
    use autome_domain::config::{self, GateConfig, GateRun, GlobalConfig, ProjectConfig};
    use automed::brief::{self, BriefMap, Inputs};
    let mut config = config::resolve(&GlobalConfig::default(), &ProjectConfig::default());
    config.gate = Some(GateConfig {
        command: "make check".into(),
        timeout_minutes: 5,
    });
    config.gate_run = GateRun::BeforeMerge;
    let old = serde_json::json!({
        "sha":"old-head", "node":"merge_gate", "command":"make check",
        "ok":false, "exit_code":1, "log":"gate.log"
    });
    let got = brief::build(&Inputs {
        role: Role::Audit,
        slug: "example",
        doc_dir: "docs/example",
        round: 2,
        status: None,
        audit_doc: None,
        evidence: &[],
        loop_protocol: "",
        session_protocol: "",
        map: &BriefMap::default(),
        budget_line: Some("本轮是第 2 轮，实现预算 N = 40。分母由 Autome 计算。\n".into()),
        decisions: None,
        gate: brief::gate_evidence(&config, Some(&old), Some("<head>")),
    });
    maybe_update_brief_golden("audit-before-merge", &got);
    assert_eq!(got, brief_golden("audit-before-merge"));
}
