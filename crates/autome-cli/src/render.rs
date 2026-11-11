//! What the command line prints.
//!
//! The same words the desktop app uses, so a person moving between the two
//! reads the same Loop: 设计、评审、裁决、实现、审计、复盘, and the two
//! stopping points named for what they wait on.

use serde_json::Value;

pub fn node_label(node: &str) -> &str {
    match node {
        "locate" => "定位",
        "repro_check" => "复现检查",
        "fix" => "修复",
        "fix_gate" => "修复门禁",
        "fix_review" => "复核",
        "await_info" => "等你补充信息",
        "not_a_bug" => "等你决定（不是 Bug）",
        "intake" => "任务整理",
        "design" => "设计",
        "review" => "评审",
        "adjudicate" => "裁决",
        "await_design_approval" => "等你批准设计",
        "implement" => "实现",
        "gate" => "门禁",
        "audit" => "审计",
        "retro" => "复盘",
        "rebase" => "rebase",
        "merge_gate" => "合并前门禁",
        "await_merge" => "等你合并",
        "merging" => "合并中",
        "main_check" => "验证 main",
        "cleanup" => "清理",
        other => other,
    }
}

pub fn milestone_label(state: &str) -> &str {
    match state {
        "open" => "开放",
        "pending" => "待审",
        "done" => "已完成",
        other => other,
    }
}

/// One phrase for a task's state, e.g. `运行中 · 实现` or `失败（审计）：…`.
pub fn state_line(state: &Value) -> String {
    let node = |key: &str| node_label(state[key].as_str().unwrap_or("?")).to_string();
    match state["state"].as_str().unwrap_or("?") {
        "draft" => "待开始".into(),
        "queued" => match state["resume"].as_str() {
            Some(resume) => format!("排队（会从 {} 继续）", node_label(resume)),
            None => "排队".into(),
        },
        "active" => {
            let n = state["node"].as_str().unwrap_or("?");
            if n.starts_with("await_") || n == "not_a_bug" {
                node_label(n).into()
            } else {
                format!("运行中 · {}", node_label(n))
            }
        }
        "paused" => format!("已暂停（会从 {} 继续）", node("resume")),
        "stopped" => format!("已停止（在 {}）", node("at")),
        "failed" => format!("失败（{}）：{}", node("at"), failure(&state["reason"])),
        "done" => "已完成".into(),
        "cancelled" => "已取消".into(),
        other => other.into(),
    }
}

fn failure(reason: &Value) -> String {
    let detail = reason["detail"].as_str().unwrap_or("");
    match reason["kind"].as_str().unwrap_or("") {
        "design_budget" => format!("设计轮用完（上限 {}）", reason["limit"]),
        "fix_budget" => format!("修复轮次用完（上限 {}）", reason["limit"]),
        "impl_budget" => format!("实现预算用完（N = {}）", reason["limit"]),
        "infeasible" => "设计文档判定不可实现".into(),
        "protocol" => format!("协议失败：{detail}"),
        "session_crashed" => format!("会话崩溃：{detail}"),
        "rebase_conflict" => format!("rebase 冲突：{detail}"),
        "config" => format!("配置问题：{detail}"),
        "core_step" => format!(
            "{} 失败：{detail}",
            node_label(reason["node"].as_str().unwrap_or("?"))
        ),
        other => other.to_string(),
    }
}

/// What the user can type next, for a task waiting on them.
pub fn next_step(task: &Value) -> Option<String> {
    let id = task["id"].as_str()?;
    let state = &task["state"];
    match (
        state["state"].as_str()?,
        state["node"].as_str().unwrap_or(""),
    ) {
        ("draft", _) => Some(format!("autome task start {id}")),
        ("active", "await_design_approval") => Some(format!(
            "autome approve {id}    或    autome reject {id} \"意见\""
        )),
        ("active", "await_info") => Some(format!(
            "autome reply {id} \"补充信息\"    或    autome cancel {id}"
        )),
        ("active", "not_a_bug") => Some(format!(
            "autome cancel {id}    或    autome task new --kind feature --brief <卡片>；确认是 Bug：autome reply {id} \"理由\""
        )),
        ("failed", _) if state["reason"]["kind"] == "fix_budget" => {
            Some(format!("autome extend {id} 1"))
        }
        ("active", "await_merge") => Some(format!("autome merge {id}")),
        ("failed", _) => Some(format!(
            "autome status {id}    （看原因，再 rerun / extend / cancel）"
        )),
        ("stopped" | "paused", _) => Some(format!("autome resume {id}")),
        _ => None,
    }
}

/// `2026-11-10T03:04:05Z` as local wall-clock time: `11:04`, or
/// `11-11 11:04` when it is not today.
pub fn local_time(iso: &str) -> String {
    let Some(epoch) = parse_iso(iso) else {
        return iso.to_string();
    };
    let (Some(then), Some(now)) = (localtime(epoch), localtime(now_epoch())) else {
        return iso.to_string();
    };
    if (then.0, then.1, then.2) == (now.0, now.1, now.2) {
        format!("{:02}:{:02}", then.3, then.4)
    } else {
        format!("{:02}-{:02} {:02}:{:02}", then.1, then.2, then.3, then.4)
    }
}

/// Seconds since the epoch for the ledger's `YYYY-MM-DDTHH:MM:SSZ`.
pub fn parse_iso(iso: &str) -> Option<i64> {
    let b = iso.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |r: std::ops::Range<usize>| iso.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // Days from civil (Howard Hinnant), the inverse of the core's formatter.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `(year, month, day, hour, minute)` in the local zone.
fn localtime(epoch: i64) -> Option<(i32, i32, i32, i32, i32)> {
    let t: libc::time_t = epoch as libc::time_t;
    // SAFETY: `localtime_r` writes into the struct we own and keeps no pointer.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&t, &mut tm).is_null() };
    ok.then_some((
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
    ))
}

/// Shortens a commit or id for a line of text.
pub fn short(s: &str) -> &str {
    &s[..s.len().min(10)]
}

/// One line about an event from the ledger, or `None` for the ones that only
/// matter to the desktop's resync. `subject` is the task (or project) id.
pub fn event_line(kind: &str, subject: &str, p: &Value) -> Option<String> {
    let s = |key: &str| p[key].as_str().unwrap_or("").to_string();
    let line = match kind {
        "bug.loop_started" => format!("{subject} 开始 Bug Loop"),
        "bug.located" => format!("{subject} 定位：{}；{}", s("status"), s("root_cause")),
        "bug.reproduced" => format!(
            "{subject} 已复现：{}（退出码 {}）",
            s("command"),
            p["exit_code"]
        ),
        "bug.repro_missed" => format!(
            "{subject} 未复现：{}；{}\n{}",
            s("command"),
            s("why"),
            s("tail")
        ),
        "bug.fix_round_started" => format!("{subject} 修复第 {} / {} 轮", p["round"], p["limit"]),
        "bug.reviewed" => format!("{subject} 复核第 {} 轮：{}", p["round"], s("verdict")),
        "task.replied" => format!("{subject} 补充信息 #{}：{}", p["n"], s("text")),
        "task.started" => format!("{subject} 开始"),
        "task.created" => format!("{subject} 新任务"),
        "task.updated" => format!("{subject} → {}", state_line(&p["state"])),
        "session.started" => {
            let label = p["kind"]
                .get("role")
                .and_then(Value::as_str)
                .map(|r| match r {
                    "plan" => "设计",
                    "review" => "评审",
                    "adjudicate" => "裁决",
                    "impl" => "实现",
                    "audit" => "审计",
                    "retro" => "复盘",
                    other => other,
                })
                .unwrap_or_else(|| {
                    p["kind"]["step"]
                        .as_str()
                        .map(node_label)
                        .unwrap_or("任务整理")
                });
            format!(
                "{subject} 开始{label}会话 #{}（{}）",
                p["round"],
                p["runtime"].as_str().unwrap_or("?")
            )
        }
        "session.retry" => {
            if p["rate_limited"].as_bool() == Some(true) {
                format!("{subject} 会话撞到{}", s("reason"))
            } else {
                format!(
                    "{subject} 会话崩溃，{} 重试（第 {}/{} 次）：{}",
                    local_time(&s("not_before")),
                    p["attempt"],
                    p["of"],
                    s("reason")
                )
            }
        }
        "session.rerouted" => format!(
            "{subject} {} 改由 {} 运行（{} 冷却到 {}）",
            s("role"),
            s("to"),
            s("from"),
            local_time(&s("cooling_until"))
        ),
        "session.deferred" => format!(
            "{subject} {} 等 {} 冷却到 {}",
            node_label(&s("node")),
            s("runtime"),
            local_time(&s("until"))
        ),
        "runtime.cooldown" => format!(
            "{} 限流，冷却到 {}：{}",
            s("runtime"),
            local_time(&s("until")),
            s("reason")
        ),
        "gate.started" => format!(
            "{subject} {}开始：{} @ {}",
            node_label(&s("node")),
            s("command"),
            short(&s("sha"))
        ),
        "gate.result" => format!(
            "{subject} {}{}（退出码 {}）  日志：{}",
            node_label(&s("node")),
            if p["ok"].as_bool() == Some(true) {
                "通过"
            } else {
                "未通过"
            },
            p["exit_code"],
            s("log")
        ),
        "gate.reused" => format!(
            "{subject} {}：提交 {} 已经通过过，沿用",
            node_label(&s("node")),
            short(&s("sha"))
        ),
        "gate.skipped" => format!("{subject} {}跳过：{}", node_label(&s("node")), s("reason")),
        "merge.auto" if p["active"].as_bool() == Some(true) => {
            let repos = p["repos"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("、");
            format!("{subject} 按合并时默认分支已前进（{repos}），自动 rebase 后再合并")
        }
        "merge.rebased" => {
            let repos = p["repos"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|repo| {
                    format!(
                        "{} → {} @ {}",
                        repo["repo"].as_str().unwrap_or(""),
                        repo["onto"].as_str().unwrap_or(""),
                        short(repo["head"].as_str().unwrap_or(""))
                    )
                })
                .collect::<Vec<_>>()
                .join("；");
            format!("{subject} 已自动 rebase：{repos}")
        }
        "project.alert" => format!("报警（{subject}）：{}  → autome alerts", s("kind")),
        "guard.warning" => format!("{subject} 守卫提醒：{}", s("code")),
        "task.protocol_failure" => format!("{subject} 协议失败：{}", s("detail")),
        _ => return None,
    };
    Some(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_automatic_rebase_is_printed_by_autome_run() {
        let line = event_line(
            "merge.auto",
            "T-9",
            &json!({"active":true,"repos":["backend","docs"]}),
        )
        .unwrap();
        assert!(line.contains("默认分支已前进"));
        assert!(line.contains("backend、docs"));
        assert_eq!(
            event_line("merge.auto", "T-9", &json!({"active":false})),
            None
        );
        for repos in [
            json!([]),
            json!([{"repo":"backend","onto":"main","head":"0123456789abcdef"}]),
            json!([{"repo":"backend","onto":"main","head":"0123456789abcdef"},{"repo":"docs","onto":"master","head":"abc"}]),
        ] {
            let line = event_line("merge.rebased", "T-9", &json!({"repos":repos})).unwrap();
            assert!(line.contains("自动 rebase"));
            if !repos.as_array().unwrap().is_empty() {
                assert!(line.contains("backend → main @ 0123456789"));
                assert!(!line.contains("0123456789abcdef"));
            }
            if repos.as_array().unwrap().len() == 2 {
                assert!(line.contains("；docs → master @ abc"));
            }
        }
    }

    #[test]
    fn requeued_states_show_the_original_node() {
        for (node, label) in [
            ("intake", "任务整理"),
            ("design", "设计"),
            ("review", "评审"),
            ("adjudicate", "裁决"),
            ("implement", "实现"),
            ("gate", "门禁"),
            ("audit", "审计"),
            ("retro", "复盘"),
            ("rebase", "rebase"),
            ("merge_gate", "合并前门禁"),
            ("merging", "合并中"),
            ("main_check", "验证 main"),
            ("cleanup", "清理"),
        ] {
            let state = json!({"state": "queued", "resume": node});
            let expected = format!("排队（会从 {label} 继续）");
            assert_eq!(state_line(&state), expected, "{node}");
            assert_eq!(
                event_line("task.updated", "T-1", &json!({"state": state})),
                Some(format!("T-1 → {expected}")),
                "{node}"
            );
        }
    }

    #[test]
    fn new_queued_states_keep_the_plain_label() {
        assert_eq!(state_line(&json!({"state": "queued"})), "排队");
        assert_eq!(
            state_line(&json!({"state": "queued", "resume": null})),
            "排队"
        );
    }

    #[test]
    fn iso_round_trips_through_the_cores_formatter() {
        for epoch in [0i64, 1_800_000_000, 951_782_400 /* 2000-02-29 */] {
            let iso = automed::store::format_iso(epoch);
            assert_eq!(parse_iso(&iso), Some(epoch), "{iso}");
        }
        assert_eq!(parse_iso("garbage"), None);
    }

    #[test]
    fn states_read_the_way_the_desktop_names_them() {
        assert_eq!(
            state_line(&json!({"state": "active", "node": "implement"})),
            "运行中 · 实现"
        );
        assert_eq!(
            state_line(&json!({"state": "active", "node": "await_merge"})),
            "等你合并"
        );
        assert_eq!(
            state_line(&json!({"state": "failed", "at": "gate",
                "reason": {"kind": "impl_budget", "limit": 25}})),
            "失败（门禁）：实现预算用完（N = 25）"
        );
    }

    #[test]
    fn a_waiting_task_says_what_to_type() {
        let t = json!({"id": "T-1", "state": {"state": "active", "node": "await_merge"}});
        assert_eq!(next_step(&t).as_deref(), Some("autome merge T-1"));
        let running = json!({"id": "T-1", "state": {"state": "active", "node": "audit"}});
        assert_eq!(next_step(&running), None);
    }
}

#[cfg(test)]
mod task_card {
    use super::*;
    use serde_json::json;
    #[test]
    fn draft_renders_as_waiting_to_start_with_the_start_command_next() {
        let t = json!({"id":"T-1","state":{"state":"draft"}});
        assert_eq!(state_line(&t["state"]), "待开始");
        assert_eq!(next_step(&t).as_deref(), Some("autome task start T-1"));
    }
    #[test]
    fn task_started_is_printed_by_run() {
        assert_eq!(
            event_line("task.started", "T-1", &json!({})).as_deref(),
            Some("T-1 开始")
        );
    }
}

#[cfg(test)]
mod bug_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn bug_loop_render_labels_every_bug_node_event_and_failure() {
        for (node, label) in [
            ("locate", "定位"),
            ("repro_check", "复现检查"),
            ("fix", "修复"),
            ("fix_gate", "修复门禁"),
            ("fix_review", "复核"),
            ("await_info", "等你补充信息"),
            ("not_a_bug", "等你决定（不是 Bug）"),
        ] {
            assert_eq!(node_label(node), label);
            assert!(state_line(&json!({"state":"active","node":node})).contains(label));
        }
        for kind in [
            "bug.loop_started",
            "bug.located",
            "bug.reproduced",
            "bug.repro_missed",
            "bug.fix_round_started",
            "bug.reviewed",
            "task.replied",
        ] {
            assert!(
                event_line(
                    kind,
                    "T-1",
                    &json!({"round":2,"limit":3,"n":1,"text":"原文"})
                )
                .is_some(),
                "{kind}"
            );
        }
        assert_eq!(
            failure(&json!({"kind":"fix_budget","limit":2})),
            "修复轮次用完（上限 2）"
        );
        assert_eq!(next_step(&json!({"id":"T-1","state":{"state":"failed","reason":{"kind":"fix_budget","limit":2}}})).as_deref(),Some("autome extend T-1 1"));
        for (step, label) in [("locate", "定位"), ("fix", "修复"), ("fix_review", "复核")] {
            assert!(
                event_line(
                    "session.started",
                    "T-1",
                    &json!({"kind":{"kind":"bug","step":step}})
                )
                .unwrap()
                .contains(label)
            );
        }
    }
}
