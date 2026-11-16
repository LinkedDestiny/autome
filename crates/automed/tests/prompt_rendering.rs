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
        integration_audit: false,
        kind,
        templates,
        gate_before_merge: None,
        gate_quick: None,
        last_gate: None,
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
        if maybe_update_golden(
            name,
            &autome_domain::protocol::select_gate_run(
                &automed::launcher::select_integration_audit(
                    automed::protocol::seed().prompt(name).unwrap(),
                    false,
                )
                .unwrap(),
                autome_domain::config::GateRun::EachRound,
            )
            .unwrap(),
        ) {
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
                "{inputs}",
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
        integration: None,
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
        scope_patches: None,
    });
    maybe_update_brief_golden("audit-before-merge", &got);
    assert_eq!(got, brief_golden("audit-before-merge"));
}

fn before_merge_rendered(kind: SessionKind, card: bool) -> String {
    let mut s = spec(kind, automed::protocol::seed());
    s.gate_before_merge = Some("make check");
    if matches!(s.kind, SessionKind::Bug { .. }) {
        s.budget = Some(automed::launcher::BudgetLine { round: 2, limit: 3 });
    }
    let path = format!("{DOC_DIR}/{SLUG}-card.md");
    if card {
        s.card_path = Some(&path);
    }
    build_prompt(&s)
        .unwrap()
        .replace(DOC_DIR, "{doc_dir}")
        .replace(SLUG, "{slug}")
        .replace(REQUEST, "{request}")
        .replace(BRIEF, "{brief_path}")
}

#[test]
fn before_merge_role_prompts_match_their_goldens() {
    for role in [Role::Plan, Role::Impl, Role::Audit] {
        let name = format!("{}-before-merge", role.as_str());
        let got = before_merge_rendered(SessionKind::Role { role }, false);
        if !maybe_update_golden(&name, &got) {
            assert_eq!(got, golden(&name));
        }
    }
}

#[test]
fn before_merge_bug_prompts_match_their_goldens() {
    use autome_domain::session::BugStep;
    for (name, step) in [("fix", BugStep::Fix), ("fix-review", BugStep::FixReview)] {
        let name = format!("{name}-before-merge");
        let got = before_merge_rendered(SessionKind::Bug { step }, false);
        if !maybe_update_golden(&name, &got) {
            assert_eq!(got, golden(&name));
        }
    }
}

#[test]
fn before_merge_leaves_the_other_prompts_byte_identical() {
    use autome_domain::session::BugStep;
    for role in [Role::Review, Role::Adjudicate] {
        assert_eq!(
            before_merge_rendered(SessionKind::Role { role }, false),
            golden(role.as_str()).replace("{budget_line}", "")
        );
    }
    for card in [false, true] {
        assert_eq!(
            before_merge_rendered(
                SessionKind::Bug {
                    step: BugStep::Locate
                },
                card
            ),
            golden(if card { "locate-card" } else { "locate" })
        );
        assert_eq!(
            before_merge_rendered(SessionKind::Intake, card),
            golden(if card { "intake-card" } else { "intake" })
        );
    }
    assert_eq!(
        before_merge_rendered(SessionKind::Onboarding, false),
        golden("onboarding")
    );
    assert_eq!(
        before_merge_rendered(SessionKind::Role { role: Role::Retro }, false),
        rendered(SessionKind::Role { role: Role::Retro })
    );
}

fn check_bug_inputs_golden(name: &str, step: autome_domain::session::BugStep) {
    let templates = automed::protocol::seed();
    let mut s = spec(SessionKind::Bug { step }, templates);
    s.budget = Some(automed::launcher::BudgetLine { round: 2, limit: 3 });
    let attachments = [
        "/tmp/autome-golden/crash.log".into(),
        "/tmp/autome-golden/screen.png".into(),
    ];
    let refs = ["docs/notes.md".into()];
    s.attachments = &attachments;
    s.doc_refs = &refs;
    let got = build_prompt(&s)
        .unwrap()
        .replace(DOC_DIR, "{doc_dir}")
        .replace(SLUG, "{slug}")
        .replace(REQUEST, "{request}");
    if maybe_update_golden(name, &got) {
        return;
    }
    assert_eq!(got, golden(name));
}

#[test]
fn bug_loop_locate_prompt_with_inputs_matches_its_golden() {
    check_bug_inputs_golden("locate-inputs", autome_domain::session::BugStep::Locate);
}
#[test]
fn bug_loop_fix_prompt_with_inputs_matches_its_golden() {
    check_bug_inputs_golden("fix-inputs", autome_domain::session::BugStep::Fix);
}
#[test]
fn bug_loop_fix_review_prompt_with_inputs_matches_its_golden() {
    check_bug_inputs_golden(
        "fix-review-inputs",
        autome_domain::session::BugStep::FixReview,
    );
}

#[test]
fn bug_inputs_and_gate_modes_preserve_user_data_independently() {
    use autome_domain::session::BugStep;
    for gate in [None, Some("check {inputs} {slug} {unknown}")] {
        for step in [BugStep::Locate, BugStep::Fix, BugStep::FixReview] {
            for (attachments, refs) in [
                (vec![], vec![]),
                (vec!["/tmp/source/screen {slug}.png".into()], vec![]),
                (vec![], vec!["docs/{gate_line}.md".into()]),
                (
                    vec!["/tmp/source/screen {slug}.png".into()],
                    vec!["docs/{gate_line}.md".into()],
                ),
            ] {
                let mut s = spec(SessionKind::Bug { step }, automed::protocol::seed());
                s.budget = Some(automed::launcher::BudgetLine { round: 2, limit: 3 });
                s.gate_before_merge = gate;
                s.attachments = &attachments;
                s.doc_refs = &refs;
                s.request = "request {inputs} {gate_line}";
                let got = build_prompt(&s).unwrap();
                assert_eq!(
                    got.matches("用户提供的附件").count(),
                    usize::from(!attachments.is_empty())
                );
                assert_eq!(
                    got.matches("用户指定的仓库内参考文档").count(),
                    usize::from(!refs.is_empty())
                );
                if !attachments.is_empty() {
                    assert!(got.contains("attachments/screen {slug}.png"));
                    assert!(!got.contains("/tmp/source"));
                }
                if !refs.is_empty() {
                    assert!(got.contains("docs/{gate_line}.md"));
                }
                if step == BugStep::Locate {
                    assert!(got.contains(s.request));
                } else if let Some(command) = gate {
                    assert!(got.contains(command));
                    assert!(!got.contains("再运行配置的项目门禁"));
                    assert!(!got.contains("独立复验复现命令与项目门禁"));
                } else {
                    assert!(!got.contains("本项目门禁只在合并前运行"));
                }
                assert!(!got.contains("<!-- gate-run:"));
            }
        }
    }
}

fn quick_golden(name: &str, kind: SessionKind) {
    let templates = automed::protocol::seed();
    let result =
        serde_json::json!({"kind":"quick","command":"( repro ) && ( make lint )","exit_code":0});
    let mut s = spec(kind, templates);
    s.gate_before_merge = Some("make check");
    s.gate_quick = Some("make lint");
    s.last_gate = Some(&result);
    s.budget = Some(automed::launcher::BudgetLine {
        round: 2,
        limit: 40,
    });
    let got = build_prompt(&s)
        .unwrap()
        .replace(DOC_DIR, "{doc_dir}")
        .replace(SLUG, "{slug}")
        .replace(REQUEST, "{request}")
        .replace(BRIEF, "{brief_path}");
    if !maybe_update_golden(name, &got) {
        assert_eq!(got, golden(name));
    }
}

#[test]
fn gate_quick_implementation_prompt_matches_golden() {
    quick_golden(
        "impl-before-merge-quick",
        SessionKind::Role { role: Role::Impl },
    );
}

#[test]
fn gate_quick_bug_prompts_match_goldens() {
    use autome_domain::session::BugStep;
    quick_golden(
        "fix-before-merge-quick",
        SessionKind::Bug { step: BugStep::Fix },
    );
    quick_golden(
        "fix-review-before-merge-quick",
        SessionKind::Bug {
            step: BugStep::FixReview,
        },
    );
}

#[test]
fn gate_quick_audit_brief_matches_golden() {
    use autome_domain::config::{
        self, GateConfig, GateRun, GlobalConfig, ProjectConfig, QuickGate,
    };
    use automed::brief::{self, BriefMap, Inputs};
    let mut config = config::resolve(&GlobalConfig::default(), &ProjectConfig::default());
    config.gate = Some(GateConfig {
        command: "make check".into(),
        timeout_minutes: 30,
    });
    config.gate_run = GateRun::BeforeMerge;
    config.gate_quick = Some(QuickGate {
        command: "make lint".into(),
        timeout_minutes: 5,
    });
    let result = serde_json::json!({"sha":"<head>","kind":"quick","command":"make lint","ok":true,"exit_code":0,"log":"gate.log"});
    let got = brief::build(&Inputs {
        integration: None,
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
        gate: brief::gate_evidence(&config, Some(&result), Some("<head>")),
        scope_patches: None,
    });
    maybe_update_brief_golden("audit-before-merge-quick", &got);
    assert_eq!(got, brief_golden("audit-before-merge-quick"));
}

#[test]
fn regression_format_only_locate_requires_precommit_check() {
    use autome_domain::session::BugStep;
    for quick in [None, Some("cargo fmt --all -- --check")] {
        let mut s = spec(
            SessionKind::Bug {
                step: BugStep::Locate,
            },
            automed::protocol::seed(),
        );
        s.budget = Some(automed::launcher::BudgetLine { round: 0, limit: 3 });
        s.gate_quick = quick;
        let prompt = build_prompt(&s).unwrap();
        for term in ["提交前", "格式化", "复现"] {
            assert!(prompt.contains(term), "定位提示词缺少 {term}");
        }
        if let Some(command) = quick {
            assert!(prompt.contains(command), "定位提示词未提供 quick 命令");
        } else {
            assert!(prompt.contains("lint"), "无 quick 时缺少项目检查指引");
        }
    }
}

#[test]
fn regression_format_only_fix_and_review_explain_exception() {
    use autome_domain::session::BugStep;
    for step in [BugStep::Fix, BugStep::FixReview] {
        for gate in [None, Some("make check")] {
            let mut s = spec(SessionKind::Bug { step }, automed::protocol::seed());
            s.budget = Some(automed::launcher::BudgetLine { round: 1, limit: 3 });
            s.gate_before_merge = gate;
            let prompt = build_prompt(&s).unwrap();
            for term in [
                "空白",
                "换行",
                "行尾逗号",
                "不算修改回归测试",
                "断言",
                "输入",
                "用例名",
                "跳过标记",
            ] {
                assert!(
                    prompt.contains(term),
                    "{step:?} 提示词缺少格式化边界：{term}"
                );
            }
        }
    }
}

fn timeout_retry_golden(name: &str, kind: SessionKind, gate: bool) {
    use autome_domain::task::Inject;
    let templates = automed::protocol::seed().clone();
    let inject = Inject::timed_out(
        5,
        "tick 39\ntick 40".into(),
        gate.then(|| Inject::GateFailed {
            digest: None,
            head: "abcdef123456".into(),
            command: "cargo test".into(),
            tail: "gate failure".into(),
        }),
    );
    let mut s = spec(kind, &templates);
    s.inject = Some(&inject);
    let card = format!("{DOC_DIR}/{SLUG}-card.md");
    let attachments = ["screen.png".into()];
    if kind == SessionKind::Intake {
        s.card_path = Some(&card);
        s.attachments = &attachments;
    }
    let got = build_prompt(&s)
        .unwrap()
        .replace(DOC_DIR, "{doc_dir}")
        .replace(SLUG, "{slug}")
        .replace(REQUEST, "{request}")
        .replace(BRIEF, "{brief_path}");
    assert!(got.contains("超过 5 分钟"));
    assert!(!got.contains("implementation-round 照常加一"));
    if kind == SessionKind::Intake {
        assert!(got.contains("screen.png"));
        assert!(got.contains("-card.md"));
    }
    if !maybe_update_golden(name, &got) {
        assert_eq!(got, golden(name));
    }
}

#[test]
fn timeout_retry_impl_golden() {
    timeout_retry_golden(
        "impl-timeout",
        SessionKind::Role { role: Role::Impl },
        false,
    );
}
#[test]
fn timeout_retry_gate_golden() {
    timeout_retry_golden(
        "impl-timeout-gate",
        SessionKind::Role { role: Role::Impl },
        true,
    );
}
#[test]
fn timeout_retry_intake_golden() {
    timeout_retry_golden("intake-timeout", SessionKind::Intake, false);
}
fn integration_prompt(before_merge: bool, name: &str) {
    let templates = automed::protocol::seed().clone();
    let mut spec = spec(SessionKind::Role { role: Role::Audit }, &templates);
    spec.integration_audit = true;
    spec.gate_before_merge = before_merge.then_some("make check");
    let got = build_prompt(&spec)
        .unwrap()
        .replace(DOC_DIR, "{doc_dir}")
        .replace(SLUG, "{slug}")
        .replace(BRIEF, "{brief_path}");
    assert!(got.contains("本轮是**集成复验**"));
    assert!(got.contains("git grep -nE"));
    assert!(!got.contains("<!-- integration-audit -->"));
    maybe_update_golden(name, &got);
    assert_eq!(got, golden(name));
}

#[test]
fn integration_audit_prompt_matches_its_golden() {
    integration_prompt(false, "audit-integration");
}
#[test]
fn integration_audit_prompt_before_merge_matches_its_golden() {
    integration_prompt(true, "audit-integration-before-merge");
}

fn integration_brief(trigger: serde_json::Value, name: &str) {
    use automed::brief::{self, BriefMap, Inputs};
    let event = serde_json::json!({"impl_round":9,"evidence":"M-08-r9-audit.md","trigger":trigger,"repos":[{"repo":"","start":"3f2a1c0123456789","end":"9e81b77123456789","base":"55d0e12123456789","commits":[{"sha":"a1b2c3d123456789","subject":"fix: resolve launcher conflict","paths":["crates/automed/src/launcher.rs"],"stat":"crates/automed/src/launcher.rs | 2 +-"},{"sha":"e4f5a6b123456789","subject":"test: keep old template compatible","paths":["crates/automed/tests/end_to_end.rs"],"stat":"crates/automed/tests/end_to_end.rs | 2 +-"}],"dropped":[{"sha":"0c9d8e7123456789","subject":"feat: launcher change","paths":["crates/automed/src/launcher.rs"],"stat":"crates/automed/src/launcher.rs | 2 ++"}],"test_files":["crates/automed/tests/end_to_end.rs"],"commits_omitted":0,"dropped_omitted":0}]});
    let got = brief::build(&Inputs {
        integration: Some(brief::integration_section(&event, "docs/example")),
        scope_patches: None,
        role: Role::Audit,
        slug: "example",
        doc_dir: "docs/example",
        round: 9,
        status: None,
        audit_doc: None,
        evidence: &[],
        loop_protocol: "",
        session_protocol: "",
        map: &BriefMap::default(),
        budget_line: None,
        decisions: None,
        gate: None,
    });
    maybe_update_brief_golden(name, &got);
    assert_eq!(got, brief_golden(name));
}
#[test]
fn integration_audit_brief_after_a_rebase_conflict_matches_its_golden() {
    integration_brief(
        serde_json::json!({"kind":"rebase_conflict","files":["crates/automed/src/launcher.rs"]}),
        "audit-integration-rebase",
    );
}
#[test]
fn integration_audit_brief_after_a_gate_failure_matches_its_golden() {
    integration_brief(
        serde_json::json!({"kind":"gate_failed","command":"make check","tail":"error: lint failed"}),
        "audit-integration-gate",
    );
}

fn operator_note_golden(name: &str, audit: bool, bug: bool) {
    let notes =
        [serde_json::json!({"n":1,"at":"2026-11-14T00:00:00Z","text":"保留原文\n检查空输入"})];
    let section = automed::handoff::note_section(&notes, audit);
    let text = if bug {
        bug_rendered(autome_domain::session::BugStep::Fix, false) + &section
    } else {
        section
    };
    if !maybe_update_golden(name, &text) {
        assert_eq!(text, golden(name));
    }
}

#[test]
fn operator_note_impl_brief_golden() {
    operator_note_golden("operator-note-impl", false, false);
}
#[test]
fn operator_note_audit_brief_golden() {
    operator_note_golden("operator-note-audit", true, false);
}
#[test]
fn operator_note_bug_fix_prompt_golden() {
    operator_note_golden("operator-note-bug", false, true);
}

fn digest_prompt(kind: SessionKind, quick: bool) {
    use autome_domain::task::Inject;
    let templates = automed::protocol::seed().clone();
    let inject = Inject::GateFailed {
        head: "abc123".into(),
        command: "check".into(),
        tail: "tail-only".into(),
        digest: Some("失败用例：assertion\n未执行用例：pending\n非用例错误：original".into()),
    };
    let result = serde_json::json!({"kind": if quick {"quick"} else {"full"}});
    let mut s = spec(kind, &templates);
    s.inject = Some(&inject);
    s.budget = Some(automed::launcher::BudgetLine {
        round: 2,
        limit: 30,
    });
    s.last_gate = Some(&result);
    let text = build_prompt(&s).unwrap();
    for expected in [
        "tail-only",
        "失败清单（宿主门禁按用例整理，原文）",
        "失败用例：assertion",
        "未执行用例：pending",
        "非用例错误：original",
    ] {
        assert!(text.contains(expected), "{text}");
    }
}
#[test]
fn digest_feature_quick_prompt() {
    digest_prompt(SessionKind::Role { role: Role::Impl }, true);
}
#[test]
fn digest_feature_merge_prompt() {
    digest_prompt(SessionKind::Role { role: Role::Impl }, false);
}
#[test]
fn digest_bug_gate_prompt() {
    for quick in [false, true] {
        digest_prompt(
            SessionKind::Bug {
                step: autome_domain::session::BugStep::Fix,
            },
            quick,
        );
    }
}
#[test]
fn digest_repair_brief_gate_line() {
    let old: autome_domain::task::Inject = serde_json::from_value(
        serde_json::json!({"inject":"gate_failed", "head":"old", "command":"check", "tail":"tail"}),
    )
    .unwrap();
    assert!(matches!(
        old,
        autome_domain::task::Inject::GateFailed { digest: None, .. }
    ));
    let old_outcome: autome_domain::task::GateOutcome = serde_json::from_value(
        serde_json::json!({"gate":"failed", "head":"old", "command":"check", "tail":"tail"}),
    )
    .unwrap();
    assert!(matches!(
        old_outcome,
        autome_domain::task::GateOutcome::Failed { digest: None, .. }
    ));
    let text = automed::brief::repair_gate_evidence(
        &serde_json::json!({"kind":"quick", "sha":"abc123", "log":"/gate.log"}),
    );
    for expected in [
        "本轮是修复轮：宿主快检",
        "abc123",
        "/gate.log",
        "失败清单与输出末尾在本轮提示词里",
    ] {
        assert!(text.contains(expected));
    }
}
