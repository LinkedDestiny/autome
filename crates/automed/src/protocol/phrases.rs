//! The sentences a protocol version must — and must not — contain.
//!
//! These used to be about twenty unit tests in `launcher.rs` and `init.rs`
//! asserting substrings of two compiled-in constants. That worked while the
//! text was a constant. Now that a user, or a meta task, can edit it, the same
//! assertions have to run against *their* version, not against the seed that
//! shipped — so they moved here and became layer 1 of `autome protocol eval`.
//!
//! Every entry carries a `why`. That is not decoration: each of these lines is
//! a rule some round invented wrongly in a real run, and the `why` is what a
//! person needs in order to decide whether a proposed edit is removing dead
//! weight or removing a scar. A requirement nobody can justify should be
//! retired through a removal experiment (plan §E3), not quietly deleted.

use autome_domain::protocol::{LOOP_PROTOCOL, ProtocolFiles, SESSION_PROTOCOL};

/// The six role templates. `intake` and `onboarding` are system steps with
/// their own shape and are not covered by the "every role" rules.
pub const ROLE_PROMPTS: [&str; 6] = [
    "prompts/plan.md",
    "prompts/review.md",
    "prompts/adjudicate.md",
    "prompts/impl.md",
    "prompts/audit.md",
    "prompts/retro.md",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    pub file: String,
    pub phrase: &'static str,
    pub why: &'static str,
}

/// `(file-or-EVERY_ROLE, phrase, why)`.
const EVERY_ROLE: &str = "*";

const REQUIRED: &[(&str, &str, &str)] = &[
    // ---- every role template -------------------------------------------
    (
        EVERY_ROLE,
        "不要启动下一个会话",
        "1.x had each round launch its successor. A round that relays bypasses the parallel limit, pause, the role switches and the budget — every guarantee the core exists to hold.",
    ),
    (
        EVERY_ROLE,
        "`协议失败` 只用于一种情况",
        "An audit round wrote 协议失败 because its sandbox refused `git commit`, and stopped a healthy task. The status has one meaning; saying so is what keeps it that way.",
    ),
    (
        EVERY_ROLE,
        "不是协议失败",
        "The other half of the same incident: the round has to be told which situations are survivable, not only which one is not.",
    ),
    (
        EVERY_ROLE,
        "git commit",
        "A real run did all its work and committed none of it. The core sweeps up afterwards, but a round that commits produces a legible history.",
    ),
    // ---- the design round ------------------------------------------------
    (
        "prompts/plan.md",
        "## 人工验收清单",
        "S6. 'End-to-end acceptance on real hardware' was made a milestone. It needs a real mouse and a real microphone, so no session could ever close it, and the budget ran out with the task sitting there.",
    ),
    (
        "prompts/plan.md",
        "不做里程碑验收条件",
        "S6's other half: it is not enough to offer the list, the round has to be told those results are not acceptance.",
    ),
    (
        "prompts/plan.md",
        "只放里程碑表那一张表格",
        "2026-09-16: a comparison table inside `## 里程碑` with a blank first header cell was read as the milestone table. The whole document failed to parse and the task stopped.",
    ),
    // ---- the implementation round ---------------------------------------
    (
        "prompts/impl.md",
        "自审清单",
        "S3. All nine implementation defects of one run were of a kind the design already listed. Each cost an implementation round and an audit round to find.",
    ),
    (
        "prompts/impl.md",
        "情形表逐行",
        "S3, item 1: walk the design's case table row by row. Skipping it is how a case the design already enumerated reaches the audit unimplemented.",
    ),
    (
        "prompts/impl.md",
        "失败分支",
        "S3, item 2: one check per failure branch the design names — denied permission, read error, timeout, empty input, service unreachable.",
    ),
    (
        "prompts/impl.md",
        "状态迁移",
        "S3, item 3: every transition, not only the happy path. State x event, channel x direction.",
    ),
    (
        "prompts/impl.md",
        "输入域边界",
        "S3, item 4: leading zeros, leap days, year boundaries, the empty string, the maximum, the single element.",
    ),
    (
        "prompts/impl.md",
        "用例总数",
        "S7a. Two full runs of the same code reported 736 and 822 cases, both with zero skipped. A skipped layer hides in pass/fail/skip; it does not hide in the total.",
    ),
    (
        "prompts/impl.md",
        "逐字搬进项目测试体系",
        "S4. The check that caught a defect has to become a regression test, or the defect is guarded only by an audit re-running it by hand every round.",
    ),
    (
        "prompts/impl.md",
        "回归用例",
        "S4: the check has to enter the project's own suite. `.autome/output/` is ignored by version control, so nothing left there survives the merge.",
    ),
    (
        "prompts/impl.md",
        "最新证据：",
        "S1. The design document takes a one-line pointer; the evidence goes in its own file.",
    ),
    (
        "prompts/impl.md",
        "不要把证据正文",
        "S1. Three runs produced design documents of 230–335KB, about seventy per cent of it appended evidence.",
    ),
    (
        "prompts/impl.md",
        "不得把里程碑标成 `已完成`",
        "The task-level form of generation/evaluation separation. Only an independent audit closes a milestone.",
    ),
    // ---- the audit round --------------------------------------------------
    (
        "prompts/audit.md",
        "结论三选一",
        "S7c. 2.0 dropped the verification-gap verdict and the audits kept reaching it anyway — 30 gaps against 14 defects in one run — with no rule to do it under.",
    ),
    (
        "prompts/audit.md",
        "验证缺口",
        "S7c: the verdict for 'the acceptance could pass a wrong implementation, but this implementation has not been shown to be wrong'.",
    ),
    (
        "prompts/audit.md",
        "不计 reopen、不退回实现轮",
        "S7c. A verification gap is not a defect; counting it as one inflates reopens and triggers convergence mode for nothing.",
    ),
    (
        "prompts/audit.md",
        "复现不了不等于不存在",
        "S7b. On concurrency and timing, 'could not reproduce, closing' misses systematically.",
    ),
    (
        "prompts/audit.md",
        "不要重跑前几轮",
        "S4. One audit re-ran 346 + 76 + 72 checks left by earlier rounds. They live in `.autome/output/`, which is gitignored, so none of them survives the merge either.",
    ),
    (
        "prompts/audit.md",
        "不要把审计结论抄进",
        "S1 for the audit side: the conclusion belongs in the audit document and the evidence file, not appended to a design document every later session re-reads.",
    ),
    // ---- the retro round --------------------------------------------------
    (
        "prompts/retro.md",
        "对着这些数字写",
        "A retro written from impression reproduces the impression. The core hands it the measured numbers so it has something to disagree with.",
    ),
    (
        "prompts/retro.md",
        "两个不同任务",
        "A lesson becomes a project rule only when a second task finds it. Saying so is what stops the round writing ten plausible-sounding singletons.",
    ),
    // ---- both loop rounds -------------------------------------------------
    (
        "prompts/impl.md",
        "轮次 | 里程碑 | 结果 | 证据 | 阻塞",
        "S2. `retro.md` reached 225KB in one run. One line a round, fixed fields.",
    ),
    (
        "prompts/audit.md",
        "轮次 | 里程碑 | 结果 | 证据 | 阻塞",
        "S2: the same one-line format on the audit side, so the run log stays a table rather than a diary.",
    ),
    // ---- the loop protocol ------------------------------------------------
    (
        LOOP_PROTOCOL,
        "实现轮不得把里程碑标成 `已完成`",
        "The rule the roles table exists to state.",
    ),
    (
        LOOP_PROTOCOL,
        "复提计数达到 2 的主张冻结为争议项",
        "Without a freezing rule the design loop can argue the same point until the round budget runs out.",
    ),
    (
        LOOP_PROTOCOL,
        "domain-review",
        "Convergence mode's first trigger: a second reopen in the same behaviour domain means the round is guessing, not converging.",
    ),
    (
        LOOP_PROTOCOL,
        "milestone-review",
        "Convergence mode's second trigger and the higher priority of the two: a third reopen of one milestone.",
    ),
    (
        LOOP_PROTOCOL,
        "Convergence Note",
        "What a converging audit has to maintain so the next round knows what is still open.",
    ),
    (
        LOOP_PROTOCOL,
        "| ID | 状态 | 标题 | reopen | 领域 |",
        "The machine-read milestone format, stated where the design round reads it.",
    ),
    (
        LOOP_PROTOCOL,
        "证据不写进设计文档",
        "S1: three runs produced 230-335KB design documents, about seventy per cent of it appended evidence, and every session reads the whole thing at every turn.",
    ),
    (
        LOOP_PROTOCOL,
        "docs/<slug>/evidence/",
        "S1: where the evidence goes instead, stated as a path so there is nothing left to interpret.",
    ),
    (
        LOOP_PROTOCOL,
        "运行记录只有一行",
        "S2: one run's `retro.md` reached 225KB, and nothing read any of it.",
    ),
    (
        LOOP_PROTOCOL,
        "标 `待审` 之前的自审清单",
        "S3: the exit condition of an implementation round, in the protocol as well as the prompt because the audit round has to know it was owed.",
    ),
    (
        LOOP_PROTOCOL,
        "审计造的检查归谁",
        "S4: whoever finds a defect does not own the regression test for it. The next implementation round does.",
    ),
    (
        LOOP_PROTOCOL,
        "搬进项目测试体系",
        "S4: the destination. A check left in `.autome/output/` guards nothing once the branch is merged.",
    ),
    (
        LOOP_PROTOCOL,
        "审计轮不重跑前几轮",
        "S4: one audit re-ran 346 + 76 + 72 checks from earlier rounds. That cost grows with the round count and protects nothing a regression test does not.",
    ),
    (
        LOOP_PROTOCOL,
        "总预算 `N` 由 Autome 计算",
        "S5. A round guessed the factor, wrote `14/14` against the core's 35, declared the budget spent — and the core scheduled another round, which then invented an explanation for the contradiction.",
    ),
    (
        LOOP_PROTOCOL,
        "验收必须是会话自己能跑的",
        "S6: an acceptance command a session cannot run produces a milestone nobody can close, and the budget runs out with the task sitting on it.",
    ),
    (
        LOOP_PROTOCOL,
        "## 人工验收清单",
        "S6: where results only a human can observe go instead. The user ticks them off before merging.",
    ),
    (
        LOOP_PROTOCOL,
        "用例总数",
        "S7a: two full runs of the same code reported 736 and 822 cases, both with zero skipped.",
    ),
    (
        LOOP_PROTOCOL,
        "复现不了不等于不存在",
        "S7b: on concurrency and timing, closing a suspected defect because it would not reproduce misses systematically.",
    ),
    (
        LOOP_PROTOCOL,
        "**验证缺口**",
        "S7c: the third verdict. 2.0 dropped it and the audits kept reaching it anyway, with no rule to do it under.",
    ),
    (
        LOOP_PROTOCOL,
        "不计 reopen，不退回实现轮",
        "S7c: a verification gap is not a defect. Counting it as one inflates reopens and trips convergence mode for nothing.",
    ),
    (
        LOOP_PROTOCOL,
        "结论三选一",
        "S7c: three verdicts, and the boundary between them is one question — did the product behave wrongly.",
    ),
    // ---- the session protocol ---------------------------------------------
    (
        SESSION_PROTOCOL,
        "`## 里程碑` 一节里只放这一张表格",
        "The 2026-09-16 parse failure, stated where the table format is defined.",
    ),
    (
        SESSION_PROTOCOL,
        "一次即判协议失败",
        "The status block is not negotiable and a near-miss is not accepted; the round has to know that before it guesses at the format.",
    ),
    (
        SESSION_PROTOCOL,
        "`协议失败` 是什么，不是什么",
        "The section that keeps the status to its one meaning.",
    ),
    (
        SESSION_PROTOCOL,
        "提交不上",
        "Not a protocol failure: Codex's `workspace-write` sandbox refuses to write the index lock, and the core commits what the session leaves behind.",
    ),
    (
        SESSION_PROTOCOL,
        "拿不到某条验收证据",
        "Not a protocol failure: record what could not be obtained and why, and let the audit round or the user verify it.",
    ),
    (
        SESSION_PROTOCOL,
        "发现了实现缺陷",
        "Not a protocol failure — that is a reopen, and the Loop keeps going.",
    ),
    (
        SESSION_PROTOCOL,
        "不要启动下一个会话",
        "The core schedules. Stated here as well as in every prompt, because this is the one rule whose violation silently disables all the others.",
    ),
    (
        SESSION_PROTOCOL,
        "| ID | 状态 | 标题 | reopen | 领域 |",
        "The machine-read format, stated where a round looks for it.",
    ),
];

/// Sentences that must *not* appear. Each one was in the text and caused a
/// specific failure.
const FORBIDDEN: &[(&str, &str, &str)] = &[
    (
        EVERY_ROLE,
        "additional task",
        "The 1.x dispatch sentence. Carrying it across cost a real run: the generated task file had no section by that name, every round opened it, found nothing addressed to itself, and did nothing. Four sessions ran and the design document was untouched.",
    ),
    (
        EVERY_ROLE,
        "没有提交的东西不会进入最终的合并",
        "False about this system — the core sweeps up what a session leaves behind. An audit round believed it and declared a protocol failure when its sandbox refused a commit.",
    ),
];

fn expand(entries: &'static [(&'static str, &'static str, &'static str)]) -> Vec<Requirement> {
    let mut out = Vec::new();
    for (file, phrase, why) in entries {
        if *file == EVERY_ROLE {
            for role_file in ROLE_PROMPTS {
                out.push(Requirement {
                    file: role_file.to_string(),
                    phrase,
                    why,
                });
            }
        } else {
            out.push(Requirement {
                file: file.to_string(),
                phrase,
                why,
            });
        }
    }
    out
}

pub fn required() -> Vec<Requirement> {
    expand(REQUIRED)
}

pub fn forbidden() -> Vec<Requirement> {
    expand(FORBIDDEN)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhraseProblem {
    /// The file is not in this version at all.
    FileMissing {
        file: String,
    },
    Missing(Requirement),
    Present(Requirement),
}

impl std::fmt::Display for PhraseProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhraseProblem::FileMissing { file } => write!(f, "本版本里没有 {file}"),
            PhraseProblem::Missing(r) => {
                write!(f, "{} 少了「{}」——{}", r.file, r.phrase, r.why)
            }
            PhraseProblem::Present(r) => {
                write!(f, "{} 里还有「{}」——{}", r.file, r.phrase, r.why)
            }
        }
    }
}

/// Layer 1's phrase check over one version.
pub fn check(files: &ProtocolFiles) -> Vec<PhraseProblem> {
    let mut out = Vec::new();
    let mut checked_files: Vec<String> = Vec::new();

    for r in required() {
        match files.get(&r.file) {
            None => {
                if !checked_files.contains(&r.file) {
                    checked_files.push(r.file.clone());
                    out.push(PhraseProblem::FileMissing {
                        file: r.file.clone(),
                    });
                }
            }
            Some(text) if !text.contains(r.phrase) => out.push(PhraseProblem::Missing(r)),
            Some(_) => {}
        }
    }
    for r in forbidden() {
        if let Some(text) = files.get(&r.file)
            && text.contains(r.phrase)
        {
            out.push(PhraseProblem::Present(r));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::seed;

    #[test]
    fn the_seed_satisfies_every_requirement() {
        let problems = check(seed());
        assert!(
            problems.is_empty(),
            "seed fails its own phrase table:\n{}",
            problems
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn deleting_a_required_sentence_is_caught_and_the_reason_is_reported() {
        let mut files = seed().clone();
        let text = files
            .loop_protocol()
            .unwrap()
            .replace("证据不写进设计文档", "");
        files.insert(LOOP_PROTOCOL, text);
        let problems = check(&files);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].to_string().contains("S1"), "{}", problems[0]);
    }

    #[test]
    fn reintroducing_a_forbidden_sentence_is_caught() {
        let mut files = seed().clone();
        let text = format!(
            "{}\n没有提交的东西不会进入最终的合并\n",
            files.prompt("impl").unwrap()
        );
        files.insert("prompts/impl.md", text);
        let problems = check(&files);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(matches!(problems[0], PhraseProblem::Present(_)));
    }

    #[test]
    fn a_missing_file_is_reported_once_rather_than_per_phrase() {
        let mut files = seed().clone();
        files.remove("prompts/audit.md");
        let problems = check(&files);
        let file_missing: Vec<_> = problems
            .iter()
            .filter(|p| matches!(p, PhraseProblem::FileMissing { .. }))
            .collect();
        assert_eq!(file_missing.len(), 1, "{problems:?}");
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    #[test]
    fn every_requirement_carries_a_reason() {
        for r in required().into_iter().chain(forbidden()) {
            assert!(
                r.why.len() > 20,
                "{} / {} has no usable reason",
                r.file,
                r.phrase
            );
        }
    }
}
