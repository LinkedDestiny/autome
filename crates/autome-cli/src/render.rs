//! What the command line prints.
//!
//! The same words the desktop app uses, so a person moving between the two
//! reads the same Loop: 设计、评审、裁决、实现、审计、复盘, and the two
//! stopping points named for what they wait on.

use serde_json::Value;

/// Convert a raw runtime line without discarding machine-only events.
pub fn stream_event(
    bytes: &[u8],
    line: usize,
    runtime: autome_domain::role::Runtime,
) -> Option<Value> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_end();
    if text.is_empty() {
        return None;
    }
    let event = serde_json::from_str::<Value>(text).unwrap_or(Value::Null);
    let rendered = match runtime {
        autome_domain::role::Runtime::Claude => automed::stream_render::render_line(text),
        autome_domain::role::Runtime::Codex => automed::stream_render::render_codex_line(text),
    };
    Some(serde_json::json!({
        "line": line, "type": event.get("type").and_then(Value::as_str),
        "text": rendered, "event": event
    }))
}

/// Running streams expose only complete lines; ended streams expose the tail.
pub fn stream_complete_bytes(bytes: &[u8], ended: bool) -> usize {
    if ended {
        bytes.len()
    } else {
        bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1)
    }
}

pub fn usd(v: f64) -> String {
    format!("${v:.2}")
}

pub fn cost_cell(task: &Value) -> String {
    let Some(cost) = task["cost_usd"].as_f64() else {
        return String::new();
    };
    let mut cell = usd(cost);
    if task["cost_sessions_without_cost"].as_u64().unwrap_or(0) > 0 {
        cell.push('*');
    }
    cell
}

#[test]
fn cost_cli_usd_has_two_decimals() {
    assert_eq!(usd(3.75), "$3.75");
    assert_eq!(usd(0.0), "$0.00");
    assert_eq!(usd(1234.5), "$1234.50");
}

#[test]
fn cost_cli_cell_marks_tasks_with_sessions_without_cost() {
    use serde_json::json;
    assert_eq!(
        cost_cell(&json!({"cost_usd":3.75,"cost_sessions_without_cost":1})),
        "$3.75*"
    );
    assert_eq!(
        cost_cell(&json!({"cost_usd":0,"cost_sessions_without_cost":0})),
        "$0.00"
    );
    assert_eq!(cost_cell(&json!({})), "");
}

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

pub fn gate_queue_line(position: u64) -> String {
    format!("排队等门禁（前面还有 {} 个）", position.saturating_sub(1))
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

pub fn gate_verdict(p: &Value) -> String {
    if p["ok"].as_bool() == Some(true) {
        return "通过".into();
    }
    autome_domain::gate_failure::kernel_timeout(p["tail"].as_str().unwrap_or("")).map_or_else(
        || "未通过".into(),
        |n| format!("门禁超时（运行超过 {n} 分钟，被内核终止）"),
    )
}

fn gate_label<'a>(p: &'a Value, missing_node: &'a str) -> &'a str {
    if p["kind"] == "quick" {
        if p["node"] == "fix_gate" {
            "修复门禁（复现 + 快检）"
        } else {
            "快检"
        }
    } else {
        node_label(p["node"].as_str().unwrap_or(missing_node))
    }
}

fn gate_kind_verdict(p: &Value) -> String {
    let verdict = gate_verdict(p);
    if p["kind"] == "quick" {
        verdict.trim_start_matches("门禁").to_string()
    } else {
        verdict
    }
}

pub fn gate_status_line(p: &Value) -> String {
    format!(
        "  门禁      {} 在 {} 上{}（{}）  日志 {}",
        gate_label(p, "?"),
        short(p["sha"].as_str().unwrap_or("")),
        gate_kind_verdict(p),
        p["command"].as_str().unwrap_or(""),
        p["log"].as_str().unwrap_or("")
    )
}

fn failure(reason: &Value) -> String {
    let detail = reason["detail"].as_str().unwrap_or("");
    match reason["kind"].as_str().unwrap_or("") {
        "design_budget" => format!("设计轮用完（上限 {}）", reason["limit"]),
        "test_fix_limit" => autome_domain::bug_loop::test_fix_limit_message(
            reason["limit"].as_u64().unwrap_or(2) as u32,
        ),
        "fix_budget" => format!("修复轮次用完（上限 {}）", reason["limit"]),
        "impl_budget" => format!("实现预算用完（N = {}）", reason["limit"]),
        "gate_same_failure"
            if reason["summary"].as_str() == Some(autome_domain::gate_failure::TIMEOUT_SUMMARY) =>
        {
            format!(
                "门禁超时：连续 {} 次运行超过时限被内核终止（日志：{}）",
                reason["count"],
                reason["log"].as_str().unwrap_or("")
            )
        }

        "gate_same_failure" => format!(
            "门禁连续 {} 次因同一原因失败：{}（日志：{}）",
            reason["count"],
            reason["summary"].as_str().unwrap_or(""),
            reason["log"].as_str().unwrap_or("")
        ),
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
            "autome task convert {id} [--start]    或    autome cancel {id}；确认是 Bug：autome reply {id} \"理由\""
        )),
        ("failed", _) if state["reason"]["kind"] == "test_fix_limit" => {
            Some(format!("autome rerun {id} locate    或取消后重提"))
        }
        ("failed", _) if state["reason"]["kind"] == "fix_budget" => {
            Some(format!("autome extend {id} 1"))
        }
        ("failed", _) if state["reason"]["kind"] == "gate_same_failure" => Some(format!(
            "autome rerun {id} {}    或    autome extend {id} 1",
            state["at"].as_str().unwrap_or("?")
        )),
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
pub fn short_state(state: &Value) -> &str {
    match state["state"].as_str().unwrap_or_default() {
        "draft" => "待开始",
        "queued" | "requeued" => "排队",
        "active" if next_step(&serde_json::json!({"id":"", "state":state})).is_some() => "等你决定",
        "active" => "运行中",
        "paused" => "已暂停",
        "stopped" => "已停止",
        "failed" => "失败",
        "done" => "已完成",
        "cancelled" => "已取消",
        _ => "未知",
    }
}

pub fn is_queued(t: &Value) -> bool {
    matches!(t["state"]["state"].as_str(), Some("queued" | "requeued"))
}

pub fn blocked_suffix(t: &Value) -> String {
    let blocked = t["blocked_by"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|d| {
                    format!(
                        "{}（{}）",
                        d["id"].as_str().unwrap_or_default(),
                        short_state(&d["state"])
                    )
                })
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default();
    if is_queued(t) && !blocked.is_empty() {
        format!("  ← 等 {blocked}")
    } else {
        String::new()
    }
}

pub fn dependencies(t: &Value) -> String {
    t["after"]
        .as_array()
        .map(|ids| {
            ids.iter()
                .map(|id| {
                    let state = t["blocked_by"]
                        .as_array()
                        .and_then(|items| items.iter().find(|d| d["id"] == *id));
                    format!(
                        "{}（{}）",
                        id.as_str().unwrap_or_default(),
                        state.map_or("已完成", |d| short_state(&d["state"]))
                    )
                })
                .collect::<Vec<_>>()
                .join("、")
        })
        .unwrap_or_default()
}

fn scope_paths(p: &Value, key: &str) -> Vec<String> {
    p["repos"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|repo| {
            if key == "patch" {
                repo[key].as_str().map(str::to_owned).into_iter().collect()
            } else {
                repo[key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            }
        })
        .collect()
}

fn scope_patch_summary(p: &Value) -> String {
    let patches = scope_paths(p, "patch");
    let first = patches.first().map(String::as_str).unwrap_or("");
    if patches.len() > 1 {
        format!("{first} 等 {} 份", patches.len())
    } else {
        first.into()
    }
}

pub fn scope_status_line(scope: &Value) -> Option<String> {
    let last = &scope["last"];
    if !last.is_object() {
        return None;
    }
    let pending = scope["pending"].as_array();
    let waiting = scope_paths(last, "patch").iter().any(|path| {
        pending.is_some_and(|items| items.iter().any(|item| item.as_str() == Some(path)))
    });
    Some(format!(
        "  越界还原：{} {}轮 #{} 还原 {} 个文件，补丁 {}（{}）",
        local_time(last["at"].as_str().unwrap_or("")),
        last["label"].as_str().unwrap_or(""),
        last["round"],
        scope_paths(last, "paths").len(),
        scope_patch_summary(last),
        if waiting { "待处理" } else { "已处理" }
    ))
}

pub fn note_budget_hint(task: &Value, notes: &Value) -> Option<String> {
    (task["state"]["reason"]["kind"] == "fix_budget"
        && notes
            .as_array()
            .is_some_and(|ns| ns.iter().any(|n| n["delivered"].is_null())))
    .then(|| {
        format!(
            "有未送达的备注；请先 autome extend {} 1",
            task["id"].as_str().unwrap_or("?")
        )
    })
}

pub fn note_status_lines(notes: &Value) -> Vec<String> {
    notes
        .as_array()
        .into_iter()
        .flatten()
        .map(|n| {
            let delivery = if n["delivered"].is_object() {
                format!(
                    "已送达 {} 第 {} 轮",
                    n["delivered"]["role"].as_str().unwrap_or(""),
                    n["delivered"]["round"]
                )
            } else {
                "未送达".to_string()
            };
            format!(
                "备注 #{} · {} · {}\n{}",
                n["n"],
                n["at"].as_str().unwrap_or(""),
                delivery,
                n["text"].as_str().unwrap_or("")
            )
        })
        .collect()
}

pub fn event_line(kind: &str, subject: &str, p: &Value) -> Option<String> {
    let s = |key: &str| p[key].as_str().unwrap_or("").to_string();
    let line = match kind {
        "task.note_added" => format!("{subject} 留下备注 #{}", p["n"]),
        "task.note_delivered" => format!(
            "{subject} 备注 {} 已送达 {} 第 {} 轮",
            p["notes"],
            s("role"),
            p["round"]
        ),
        "session.scope_restored" => {
            let paths = scope_paths(p, "paths");
            let mut listed = paths.iter().take(3).cloned().collect::<Vec<_>>().join("、");
            if paths.len() > 3 {
                listed.push_str(&format!(" 等 {} 个", paths.len()));
            }
            let next = if s("label") == "复核" {
                "修复"
            } else {
                "实现"
            };
            format!(
                "{subject} {}轮 #{} 越界改动已自动还原：{} 个文件（{}），补丁 {}；交给下一个{next}轮决定是否采纳",
                s("label"),
                p["round"],
                paths.len(),
                listed,
                scope_patch_summary(p)
            )
        }
        "session.scope_restore_failed" => {
            format!("{subject} 越界改动自动还原失败：{}", s("detail"))
        }
        "session.scope_patches_handled" => format!(
            "{subject} 越界补丁已交给会话 {} 处理（{} 份）",
            s("session_id"),
            p["patches"].as_array().map_or(0, Vec::len)
        ),
        "bug.loop_started" => format!("{subject} 开始 Bug Loop"),
        "bug.located" => format!("{subject} 定位：{}；{}", s("status"), s("root_cause")),
        "bug.reproduced" => format!(
            "{subject} 已复现：{}（退出码 {}）",
            s("command"),
            p["exit_code"]
        ),
        "bug.repro_missed" if p["paired"].is_object() => format!(
            "{subject} 修正后的回归测试：基线上没有失败，按未复现处理；{}\n{}",
            s("why"),
            s("tail")
        ),
        "bug.repro_missed"
            if s("why") == "timed_out"
                && autome_domain::gate_failure::kernel_timeout(&s("tail")).is_some() =>
        {
            format!(
                "{subject} 门禁超时：{} 运行超过 {} 分钟被内核终止，本次不能据此断言未复现\n{}",
                s("command"),
                autome_domain::gate_failure::kernel_timeout(&s("tail")).unwrap(),
                s("tail")
            )
        }

        "bug.repro_missed" => format!(
            "{subject} 未复现：{}；{}\n{}",
            s("command"),
            s("why"),
            s("tail")
        ),
        "bug.test_fix_requested" => format!(
            "{subject} 回归测试被判有缺陷，退回定位轮修正（第 {} / {} 次）",
            p["attempt"], p["limit"]
        ),
        "bug.test_fix_settled" => format!(
            "{subject} 修正后的回归测试：{}",
            match p["result"].as_str() {
                Some("accepted") => "基线上失败、当前分支通过，接受",
                Some("needs_fix") => "基线上失败、当前分支仍失败，回到修复轮",
                _ => "基线上没有失败，按未复现处理",
            }
        ),
        "bug.fix_round_started" => format!("{subject} 修复第 {} / {} 轮", p["round"], p["limit"]),
        "bug.reviewed" => format!("{subject} 复核第 {} 轮：{}", p["round"], s("verdict")),
        "task.replied" => format!("{subject} 补充信息 #{}：{}", p["n"], s("text")),
        "task.started" => format!("{subject} 开始"),
        "task.swept" => format!(
            "{subject} 兜底提交 {}：新增跟踪 {} 个文件（最多列 20 个）：{}",
            s("sha"),
            p["added_count"],
            p["added_paths"]
        ),
        "task.blocked" => format!(
            "{subject} 被阻塞：等 {}（{}）",
            s("on"),
            if p["reason"] == "cancelled" {
                "已取消"
            } else {
                "失败"
            }
        ),
        "task.moved" => {
            if p["top"] == true {
                format!("{subject} 移到队首")
            } else if p["before"].is_string() {
                format!("{subject} 移到 {} 之前", s("before"))
            } else {
                format!("{subject} 移到 {} 之后", s("after"))
            }
        }
        "task.after_set" => {
            let ids = p["after"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("、")
                })
                .unwrap_or_default();
            if ids.is_empty() {
                format!("{subject} 依赖已清空")
            } else {
                format!("{subject} 依赖改为 {ids}")
            }
        }
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
        "session.timeout" => format!(
            "{subject} 会话运行 {} 分钟，超过上限 {}，已终止",
            p["minutes"], p["limit"]
        ),
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
        "merge.after_started" => format!(
            "{subject} 合并后命令开始：{}  日志：{}",
            s("command"),
            s("log")
        ),
        "merge.after" => format!(
            "{subject} 合并后命令{}（退出码 {}）  日志：{}",
            if p["ok"].as_bool() == Some(true) {
                "通过"
            } else {
                "未通过"
            },
            p["exit_code"],
            s("log")
        ),
        "merge.after_skipped" => format!("{subject} 合并后命令跳过：{}", s("reason")),
        "setup.started" => format!(
            "{subject} 准备命令开始（{}）：{}  日志：{}",
            setup_checkout_label(&s("checkout")),
            s("command"),
            s("log")
        ),
        "setup.result" => format!(
            "{subject} 准备命令{}（{}，退出码 {}）  日志：{}",
            if p["ok"].as_bool() == Some(true) {
                "通过"
            } else {
                "未通过"
            },
            setup_checkout_label(&s("checkout")),
            p["exit_code"],
            s("log")
        ),
        "setup.skipped" => format!("{subject} 准备命令跳过：{}", s("reason")),
        "gate.queued" => format!(
            "{subject} 排队等门禁（{}，入队时前面还有 {} 个）",
            node_label(&s("node")),
            p["position"].as_u64().unwrap_or(1).saturating_sub(1)
        ),
        "gate.started" => format!(
            "{subject} {}开始：{} @ {}",
            gate_label(p, ""),
            s("command"),
            short(&s("sha"))
        ),
        "gate.result" => format!(
            "{subject} {}{}（退出码 {}）  日志：{}",
            gate_label(p, ""),
            gate_kind_verdict(p),
            p["exit_code"],
            s("log")
        ),
        "gate.reused" if p["node"] == "main_check" => {
            format!("{subject} 验证 main 沿用合并前门禁的结果（内容相同）")
        }
        "gate.reused" => format!(
            "{subject} {}：提交 {} 已经通过过，沿用",
            node_label(&s("node")),
            short(&s("sha"))
        ),
        "gate.skipped" => format!("{subject} {}跳过：{}", node_label(&s("node")), s("reason")),
        "merge.locked" => {
            format!("{subject} 进入合并临界区：门禁通过即合并，期间本项目不开始别的合并")
        }
        "merge.unlocked" => format!("{subject} 离开合并临界区"),
        "merge.waiting" => format!(
            "{subject} 等 {} 合并完成后{}",
            s("holder"),
            if p["node"] == "merging" {
                "再合并"
            } else {
                "再开跑门禁"
            }
        ),
        "merge.direct" => format!(
            "{subject} 门禁通过，直接合并（已授权：{}）",
            match p["by"].as_str() {
                Some("user") => "你预先同意了",
                Some("project") => "项目开启了自动合并",
                _ => "按合并时分支落后",
            }
        ),
        "merge.auto" if p["by"] == "user" => format!(
            "{subject} {}",
            if p["active"] == true {
                "已预先同意合并：走到合并卡点时直接合并"
            } else {
                "已撤销预先同意合并"
            }
        ),
        "merge.rebased" if p["at"] == "dequeue" => {
            let repos = p["repos"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|repo| {
                    let name = repo["repo"].as_str().unwrap_or("");
                    let prefix = if name.is_empty() {
                        String::new()
                    } else {
                        format!("{name} → ")
                    };
                    format!(
                        "{prefix}{} @ {}",
                        repo["onto"].as_str().unwrap_or(""),
                        short(repo["base_sha"].as_str().unwrap_or(""))
                    )
                })
                .collect::<Vec<_>>()
                .join("；");
            format!("{subject} 出队时已 rebase 到 {repos}")
        }
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
            if let Some([repo]) = p["repos"].as_array().map(Vec::as_slice)
                && repo["repo"].as_str() == Some("")
            {
                return Some(format!(
                    "{subject} 已自动 rebase 到 {} @ {}",
                    repo["onto"].as_str().unwrap_or(""),
                    short(repo["head"].as_str().unwrap_or(""))
                ));
            }
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
        "task.budget_exceeded" => format!(
            "{subject} 累计费用 ${:.2} 超过预算 ${:.2}，任务照常继续  → autome alerts",
            p["usd_total"].as_f64().unwrap_or(0.0),
            p["limit_usd"].as_f64().unwrap_or(0.0)
        ),
        "project.alert" if s("kind") == "budget_exceeded" => format!(
            "报警（{subject}）：{} 费用超出预算  → autome alerts",
            s("task_id")
        ),
        "project.alert" => format!("报警（{subject}）：{}  → autome alerts", s("kind")),
        "guard.warning" => format!("{subject} 守卫提醒：{}", s("code")),
        "task.protocol_failure" => format!("{subject} 协议失败：{}", s("detail")),
        _ => return None,
    };
    Some(line)
}

/// Shared by list, dashboard status and task details. Old servers omit `merge`.
pub fn merge_suffix(task: &Value, compact: bool) -> String {
    let m = &task["merge"];
    let mut parts = Vec::new();
    if let Some(n) = m["stale"]["behind"].as_u64().filter(|n| *n > 0) {
        let gated = m["stale"]["gated"] == true;
        let bases = m["stale"]["repos"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r["base"].as_str())
            .collect::<Vec<_>>()
            .join("、");
        if compact {
            parts.push(format!(
                "{}（落后 {n}）",
                if gated {
                    "结果已过期"
                } else {
                    "分支已落后"
                }
            ));
        } else if gated {
            parts.push(format!(
                "门禁结果基于旧的 {bases}（落后 {n} 个提交），合并时会重新 rebase 并再跑一次门禁"
            ));
        } else {
            parts.push(format!("分支落后 {bases} {n} 个提交，合并时会先 rebase"));
        }
    }
    if m["authorized"] == true {
        parts.push(
            if m["authorized_by"] == "project" {
                "项目已开启自动合并"
            } else if compact {
                "已同意合并"
            } else {
                "已预先同意合并"
            }
            .into(),
        );
    }
    if let Some(holder) = m["waiting_for"].as_str() {
        parts.push(format!(
            "等 {holder} 合并{}",
            if compact { "" } else { "完成" }
        ));
    }
    parts.into_iter().map(|p| format!(" · {p}")).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn timeout_visible_event_line() {
        assert_eq!(
            super::event_line(
                "session.timeout",
                "T-1",
                &serde_json::json!({"session_id":"s1","minutes":37,"limit":5})
            )
            .as_deref(),
            Some("T-1 会话运行 37 分钟，超过上限 5，已终止")
        );
    }

    #[test]
    fn gate_quick_run_lines_name_the_quick_run() {
        for (node, old, quick) in [
            ("gate", "门禁", "快检"),
            ("fix_gate", "修复门禁", "修复门禁（复现 + 快检）"),
        ] {
            for kind in [
                None,
                Some("full"),
                Some("repro"),
                Some("unknown"),
                Some("quick"),
            ] {
                for (ok, tail, verdict, code) in [
                    (true, "", "通过", 0),
                    (false, "exit 124", "未通过", 124),
                    (
                        false,
                        "=== Autome：门禁运行超过 3 分钟，已终止 ===",
                        "超时（运行超过 3 分钟，被内核终止）",
                        124,
                    ),
                ] {
                    let mut p = json!({"node":node,"sha":"123456789abcdef","command":"check","log":"/log","ok":ok,"tail":tail,"exit_code":code});
                    if let Some(k) = kind {
                        p["kind"] = json!(k);
                    }
                    let label = if kind == Some("quick") { quick } else { old };
                    let result = if kind != Some("quick") && verdict.starts_with("超时") {
                        format!("门禁{verdict}")
                    } else {
                        verdict.into()
                    };
                    assert_eq!(
                        event_line("gate.started", "T", &p).unwrap(),
                        format!("T {label}开始：check @ 123456789a")
                    );
                    assert_eq!(
                        event_line("gate.result", "T", &p).unwrap(),
                        format!("T {label}{result}（退出码 {code}）  日志：/log")
                    );
                }
            }
        }
    }

    #[test]
    fn gate_quick_status_line_names_the_kind() {
        for (node, old, quick) in [
            ("gate", "门禁", "快检"),
            ("fix_gate", "修复门禁", "修复门禁（复现 + 快检）"),
        ] {
            for kind in [
                None,
                Some("full"),
                Some("repro"),
                Some("unknown"),
                Some("quick"),
            ] {
                for (ok, tail, verdict) in [
                    (true, "", "通过"),
                    (false, "", "未通过"),
                    (
                        false,
                        "=== Autome：门禁运行超过 3 分钟，已终止 ===",
                        "超时（运行超过 3 分钟，被内核终止）",
                    ),
                ] {
                    let mut p = json!({"node":node,"sha":"123456789abcdef","command":"check","log":"/log","ok":ok,"tail":tail});
                    if let Some(k) = kind {
                        p["kind"] = json!(k);
                    }
                    let label = if kind == Some("quick") { quick } else { old };
                    let result = if kind != Some("quick") && verdict.starts_with("超时") {
                        format!("门禁{verdict}")
                    } else {
                        verdict.into()
                    };
                    assert_eq!(
                        gate_status_line(&p),
                        format!("  门禁      {label} 在 123456789a 上{result}（check）  日志 /log")
                    );
                }
            }
        }
        assert_eq!(
            gate_status_line(&json!({"kind":"quick"})),
            "  门禁      快检 在  上未通过（）  日志 "
        );
    }

    #[test]
    fn gate_queue_line_counts_only_waiters_ahead() {
        for (position, ahead) in [(1, 0), (2, 1), (u64::MAX, u64::MAX - 1), (0, 0)] {
            assert_eq!(
                gate_queue_line(position),
                format!("排队等门禁（前面还有 {ahead} 个）")
            );
        }
    }

    #[test]
    fn gate_queue_event_labels_the_node_and_snapshot() {
        for (node, label) in [
            ("gate", "门禁"),
            ("merge_gate", "合并前门禁"),
            ("main_check", "验证 main"),
            ("repro_check", "复现检查"),
            ("fix_gate", "修复门禁"),
            ("implement", "实现"),
            ("cleanup", "清理"),
        ] {
            for (position, ahead) in [(1, 0), (3, 2)] {
                assert_eq!(
                    event_line(
                        "gate.queued",
                        "T-1",
                        &json!({"node":node,"position":position})
                    ),
                    Some(format!(
                        "T-1 排队等门禁（{label}，入队时前面还有 {ahead} 个）"
                    ))
                );
            }
        }
        assert!(event_line("gate.queue_dropped", "T-1", &json!({})).is_none());
        assert!(event_line("bug.fix_round_requested", "T-1", &json!({})).is_none());
    }

    #[test]
    fn gate_queue_state_line_keeps_the_original_node() {
        for (node, label) in [
            ("gate", "门禁"),
            ("merge_gate", "合并前门禁"),
            ("main_check", "验证 main"),
            ("repro_check", "复现检查"),
            ("fix_gate", "修复门禁"),
        ] {
            assert_eq!(
                state_line(&json!({"state":"active","node":node})),
                format!("运行中 · {label}")
            );
        }
    }

    #[test]
    fn task_convert_is_the_next_step_for_not_a_bug() {
        let task = serde_json::json!({
            "id": "T-42", "state": {"state": "active", "node": "not_a_bug"}
        });
        let next = super::next_step(&task).unwrap();
        assert!(next.contains("autome task convert T-42 [--start]"));
        assert!(next.contains("autome cancel T-42"));
        assert!(next.contains("autome reply T-42"));
        assert!(!next.contains("task new --kind feature --brief"));
    }

    #[test]
    fn merge_after_events_are_printed_by_autome_run() {
        use serde_json::json;
        let p = json!({"command":"sh deploy.sh", "log":"hook.log", "ok":true, "exit_code":0, "reason":"没有单一的合并提交"});
        assert_eq!(
            super::event_line("merge.after_started", "T-1", &p).unwrap(),
            "T-1 合并后命令开始：sh deploy.sh  日志：hook.log"
        );
        assert_eq!(
            super::event_line("merge.after", "T-1", &p).unwrap(),
            "T-1 合并后命令通过（退出码 0）  日志：hook.log"
        );
        let mut p = p;
        p["ok"] = json!(false);
        p["exit_code"] = json!(6);
        assert_eq!(
            super::event_line("merge.after", "T-1", &p).unwrap(),
            "T-1 合并后命令未通过（退出码 6）  日志：hook.log"
        );
        assert_eq!(
            super::event_line("merge.after_skipped", "T-1", &p).unwrap(),
            "T-1 合并后命令跳过：没有单一的合并提交"
        );
    }

    #[test]
    fn sweep_event_prints_count_and_escaped_paths_on_one_line() {
        let line = super::event_line(
            "task.swept",
            "T-1",
            &serde_json::json!({
                "sha": "abc123", "added_count": 21, "added_paths": ["source.rs", "line\nbreak.txt"]
            }),
        )
        .unwrap();
        assert!(line.contains("21 个文件"));
        assert!(line.contains("source.rs"));
        assert_eq!(line.lines().count(), 1);
    }

    #[test]
    fn gate_timeout_result_event() {
        for tail in ["=== Autome：门禁运行超过 3 分钟，已终止 ===", "exit 124"] {
            let p = json!({"node":"gate","ok":false,"exit_code":124,"tail":tail,"log":"/log"});
            let line = event_line("gate.result", "T", &p).unwrap();
            assert_eq!(line.contains("门禁超时"), tail.starts_with("==="));
            assert!(line.contains("/log"));
        }
    }
    #[test]
    fn gate_timeout_status_verdict() {
        assert_eq!(
            gate_verdict(&json!({"ok":false,"tail":"=== Autome：门禁运行超过 3 分钟，已终止 ==="})),
            "门禁超时（运行超过 3 分钟，被内核终止）"
        );
        assert_eq!(gate_verdict(&json!({"ok":false,"exit_code":124})), "未通过");
        assert_eq!(gate_verdict(&json!({"ok":true})), "通过");
    }
    #[test]
    fn gate_timeout_same_failure() {
        let line = failure(
            &json!({"kind":"gate_same_failure","summary":autome_domain::gate_failure::TIMEOUT_SUMMARY,"count":3,"log":"/log"}),
        );
        assert_eq!(
            line,
            "门禁超时：连续 3 次运行超过时限被内核终止（日志：/log）"
        );
    }
    #[test]
    fn gate_timeout_repro_event() {
        for why in ["timed_out", "passed"] {
            for tail in ["=== Autome：门禁运行超过 3 分钟，已终止 ===", "exit 124"] {
                let line = event_line(
                    "bug.repro_missed",
                    "T",
                    &json!({"why":why,"tail":tail,"command":"check"}),
                )
                .unwrap();
                let timeout = why == "timed_out" && tail.starts_with("===");
                assert_eq!(line.contains("不能据此断言未复现"), timeout);
                assert!(line.contains(tail));
                if !timeout {
                    assert!(line.starts_with("T 未复现：check；"));
                }
            }
        }
    }

    use super::*;
    use serde_json::json;

    #[test]
    fn rebased_event_single_repo_has_no_empty_repository_label() {
        let line = event_line(
            "merge.rebased",
            "T-14",
            &json!({"repos":[{"repo":"","onto":"main","head":"5c8631f6a5abcdef01"}]}),
        )
        .unwrap();
        assert_eq!(line, "T-14 已自动 rebase 到 main @ 5c8631f6a5");
    }

    #[test]
    fn rebased_event_workspace_preserves_repository_labels() {
        for (repos, expected) in [
            (
                json!([{"repo":"backend","onto":"main","head":"0123456789abcdef"}]),
                "T-14 已自动 rebase：backend → main @ 0123456789",
            ),
            (
                json!([
                    {"repo":"backend","onto":"main","head":"0123456789abcdef"},
                    {"repo":"docs","onto":"master","head":"abcdef0123456789"}
                ]),
                "T-14 已自动 rebase：backend → main @ 0123456789；docs → master @ abcdef0123",
            ),
        ] {
            assert_eq!(
                event_line("merge.rebased", "T-14", &json!({"repos":repos})).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn setup_events_are_printed_by_autome_run() {
        for (checkout, label) in [
            ("worktree", "任务检出"),
            ("verify", "合并提交检出"),
            ("main", "主检出"),
        ] {
            let p = json!({"checkout":checkout,"command":"npm ci","log":"/tmp/setup.log"});
            let start = event_line("setup.started", "T-1", &p).unwrap();
            assert!(
                start.contains(label)
                    && start.contains("npm ci")
                    && start.contains("/tmp/setup.log")
            );
            for ok in [true, false] {
                let line = event_line("setup.result", "T-1", &json!({
                    "checkout":checkout,"ok":ok,"exit_code":if ok {0} else {7},"log":"/tmp/setup.log"
                })).unwrap();
                assert!(line.contains(label) && line.contains("/tmp/setup.log"));
                assert!(line.contains(if ok {
                    "准备命令通过"
                } else {
                    "准备命令未通过"
                }));
                assert!(line.contains(if ok { "退出码 0" } else { "退出码 7" }));
            }
        }
        assert!(
            event_line(
                "setup.skipped",
                "T-1",
                &json!({"reason":"工作区项目不支持准备命令"})
            )
            .unwrap()
            .contains("准备命令跳过：工作区项目不支持准备命令")
        );
    }

    #[test]
    fn main_check_reuse_is_printed_by_autome_run() {
        for (node, expected) in [
            (
                "main_check",
                "T-1 验证 main 沿用合并前门禁的结果（内容相同）",
            ),
            (
                "merge_gate",
                "T-1 合并前门禁：提交 123456789a 已经通过过，沿用",
            ),
        ] {
            assert_eq!(
                event_line("gate.reused", "T-1", &json!({
                    "node": node, "sha": "123456789abcdef", "from_sha": "abcdef", "tree": "fedcba"
                })).as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn gate_same_failure_explains_the_reason_and_log() {
        for count in [1, 3, 10] {
            let reason = json!({"kind":"gate_same_failure","count":count,"summary":"错误 S","log":"/日志/gate.log"});
            assert_eq!(
                failure(&reason),
                format!("门禁连续 {count} 次因同一原因失败：错误 S（日志：/日志/gate.log）")
            );
        }
    }

    #[test]
    fn gate_same_failure_offers_rerun_and_extend() {
        for at in ["gate", "merge_gate", "fix_gate"] {
            let task = json!({"id":"T-9","state":{"state":"failed","at":at,"reason":{"kind":"gate_same_failure","count":3,"summary":"S","log":"/l"}}});
            assert_eq!(
                next_step(&task),
                Some(format!(
                    "autome rerun T-9 {at}    或    autome extend T-9 1"
                ))
            );
        }
    }

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

fn setup_checkout_label(checkout: &str) -> &str {
    match checkout {
        "worktree" => "任务检出",
        "verify" => "合并提交检出",
        "main" => "主检出",
        other => other,
    }
}

#[cfg(test)]
mod stream_tests {
    use super::*;
    use autome_domain::role::Runtime::{Claude, Codex};

    #[test]
    fn stream_claude() {
        let raw =
            br#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi there"}]}}"#;
        let got = stream_event(raw, 2, Claude).unwrap();
        assert_eq!(got["line"], 2);
        assert_eq!(got["type"], "assistant");
        assert_eq!(got["text"], "hi there");
        assert_eq!(got["event"], serde_json::from_slice::<Value>(raw).unwrap());
        assert!(stream_event(br#"{"type":"system","subtype":"hook_started"}"#, 3, Claude).unwrap()["text"].is_null());
    }

    #[test]
    fn stream_codex() {
        let raw = br#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"Hello!"}}"#;
        assert_eq!(stream_event(raw, 1, Codex).unwrap()["text"], "Hello!");
        assert!(stream_event(br#"{"type":"turn.started"}"#, 2, Codex).unwrap()["text"].is_null());
    }

    #[test]
    fn stream_text_and_input_boundaries() {
        let got = stream_event(b"warning\r\n", 1, Claude).unwrap();
        assert_eq!(
            got,
            serde_json::json!({"line":1,"type":null,"text":"warning","event":null})
        );
        assert!(stream_event(b" \r\n", 2, Claude).is_none());
        assert!(stream_event(b"", 3, Codex).is_none());
        assert_eq!(stream_event(b"\xff", 4, Codex).unwrap()["text"], "\u{fffd}");
        for raw in ["null", "42", "[]", "{\"type\":3}"] {
            assert!(stream_event(raw.as_bytes(), 5, Claude).unwrap()["type"].is_null());
        }
    }

    #[test]
    fn stream_line_boundaries() {
        let raw = b"one\ntwo\nthree";
        assert_eq!(stream_complete_bytes(raw, false), 8);
        assert_eq!(stream_complete_bytes(&raw[8..], false), 0);
        assert_eq!(stream_complete_bytes(&raw[8..], true), 5);
        assert_eq!(stream_complete_bytes(b"", true), 0);
        assert_eq!(stream_complete_bytes(b"\n", false), 1);
        assert_eq!(stream_complete_bytes(b"\xc3", false), 0);
    }
}

#[test]
fn budget_alert_event_and_alert_lines() {
    use serde_json::json;
    assert_eq!(
        event_line(
            "task.budget_exceeded",
            "T-9",
            &json!({"usd_total": 0.5, "limit_usd": 0.4})
        )
        .unwrap(),
        "T-9 累计费用 $0.50 超过预算 $0.40，任务照常继续  → autome alerts"
    );
    assert_eq!(
        event_line(
            "project.alert",
            "p",
            &json!({"kind": "budget_exceeded", "task_id": "T-9"})
        )
        .unwrap(),
        "报警（p）：T-9 费用超出预算  → autome alerts"
    );
    assert_eq!(
        event_line("project.alert", "p", &json!({"kind": "main_gate_failed"})).unwrap(),
        "报警（p）：main_gate_failed  → autome alerts"
    );
}

#[cfg(test)]
mod merge_view_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn merge_view_event_lines_for_the_merge_queue() {
        for (kind, payload, expected) in [
            ("merge.locked", json!({}), "进入合并临界区"),
            ("merge.unlocked", json!({}), "离开合并临界区"),
            (
                "merge.waiting",
                json!({"node":"merge_gate","holder":"T-2"}),
                "等 T-2 合并完成后再开跑门禁",
            ),
            (
                "merge.waiting",
                json!({"node":"merging","holder":"T-2"}),
                "等 T-2 合并完成后再合并",
            ),
            (
                "merge.auto",
                json!({"active":true,"by":"user"}),
                "已预先同意合并",
            ),
            (
                "merge.auto",
                json!({"active":false,"by":"user"}),
                "已撤销预先同意合并",
            ),
        ] {
            assert!(
                event_line(kind, "T-1", &payload)
                    .unwrap()
                    .contains(expected)
            );
        }
        for (by, expected) in [
            ("user", "你预先同意了"),
            ("project", "项目开启了自动合并"),
            ("behind", "按合并时分支落后"),
        ] {
            assert!(
                event_line("merge.direct", "T-1", &json!({"by":by}))
                    .unwrap()
                    .contains(expected)
            );
        }
        for repos in [
            json!([{"repo":"","onto":"main","base_sha":"abcdef123456","head":"wrong"}]),
            json!([{"repo":"api","onto":"main","base_sha":"abcdef123456"},{"repo":"web","onto":"master","base_sha":"123456abcdef"}]),
        ] {
            let line = event_line(
                "merge.rebased",
                "T-1",
                &json!({"at":"dequeue","repos":repos}),
            )
            .unwrap();
            assert!(line.contains("出队时已 rebase 到"));
            assert!(line.contains("main @ abcdef"));
            assert!(!line.contains("wrong"));
        }
    }
    #[test]
    fn merge_view_state_suffixes_for_ls_and_status() {
        assert_eq!(merge_suffix(&json!({}), false), "");
        for gated in [true, false] {
            let task = json!({"merge":{"authorized":true,"authorized_by":"user","waiting_for":"T-2","stale":{"behind":1,"gated":gated,"repos":[{"base":"main"}]}}});
            let full = merge_suffix(&task, false);
            assert!(full.contains("已预先同意合并"));
            assert!(full.contains("等 T-2 合并完成"));
            assert_eq!(full.contains("再跑一次门禁"), gated);
            assert!(full.contains("main"));
            let short = merge_suffix(&task, true);
            assert!(short.contains("落后 1"));
            assert!(short.contains("已同意合并"));
        }
        assert!(
            merge_suffix(
                &json!({"merge":{"authorized":true,"authorized_by":"project"}}),
                false
            )
            .contains("项目已开启自动合并")
        );
        assert_eq!(
            merge_suffix(&json!({"merge":{"stale":{"behind":0}}}), true),
            ""
        );
    }
}

#[test]
fn deps_cli_events() {
    use serde_json::json;
    for (kind, payload, expected) in [
        (
            "task.blocked",
            json!({"on":"T-2","reason":"cancelled"}),
            "T-1 被阻塞：等 T-2（已取消）",
        ),
        (
            "task.blocked",
            json!({"on":"T-2","reason":"failed"}),
            "T-1 被阻塞：等 T-2（失败）",
        ),
        ("task.moved", json!({"top":true}), "T-1 移到队首"),
        ("task.moved", json!({"before":"T-2"}), "T-1 移到 T-2 之前"),
        ("task.moved", json!({"after":"T-2"}), "T-1 移到 T-2 之后"),
        (
            "task.after_set",
            json!({"after":["T-2","T-3"]}),
            "T-1 依赖改为 T-2、T-3",
        ),
        ("task.after_set", json!({"after":[]}), "T-1 依赖已清空"),
    ] {
        assert_eq!(event_line(kind, "T-1", &payload).as_deref(), Some(expected));
    }
    for (state, expected) in [
        ("draft", "待开始"),
        ("queued", "排队"),
        ("requeued", "排队"),
        ("active", "运行中"),
        ("paused", "已暂停"),
        ("stopped", "已停止"),
        ("failed", "失败"),
        ("done", "已完成"),
        ("cancelled", "已取消"),
    ] {
        assert_eq!(short_state(&json!({"state":state})), expected);
    }
    assert_eq!(
        short_state(&json!({"state":"active","node":"await_design_approval"})),
        "等你决定"
    );
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    use serde_json::json;

    fn payload() -> Value {
        json!({"label":"审计", "round":3, "repos":[
            {"paths":["a.rs", "中文 空格.rs", "x[1]*.rs"], "patch":"/tmp/a.patch"},
            {"paths":["hidden.rs"], "patch":"/tmp/b.patch"}
        ]})
    }

    #[test]
    fn scope_restored_event_lists_bounded_paths_and_patches() {
        let mut p = payload();
        let line = event_line("session.scope_restored", "T-9", &p).unwrap();
        assert_eq!(
            line,
            "T-9 审计轮 #3 越界改动已自动还原：4 个文件（a.rs、中文 空格.rs、x[1]*.rs 等 4 个），补丁 /tmp/a.patch 等 2 份；交给下一个实现轮决定是否采纳"
        );
        p["repos"].as_array_mut().unwrap().pop();
        let line = event_line("session.scope_restored", "T-9", &p).unwrap();
        assert!(!line.contains(" 等 "));
        for label in ["复核", "复盘"] {
            p["label"] = json!(label);
            p["round"] = json!(u32::MAX);
            p["repos"][0]["paths"] = json!(["单个"]);
            let line = event_line("session.scope_restored", "T-9", &p).unwrap();
            assert!(line.contains(&format!("{label}轮 #4294967295")));
            assert!(line.contains("1 个文件（单个）"));
            assert!(line.contains(if label == "复核" {
                "下一个修复轮"
            } else {
                "下一个实现轮"
            }));
        }
        assert!(event_line("session.scope_restore_begun", "T-9", &p).is_none());
    }

    #[test]
    fn scope_failed_event_preserves_detail() {
        for detail in [
            "只能修改 docs/t/。先保存补丁再 rerun。上一次已自动还原",
            "自动还原没有完成：读盘失败",
            "",
        ] {
            assert_eq!(
                event_line(
                    "session.scope_restore_failed",
                    "T-9",
                    &json!({"detail":detail})
                )
                .unwrap(),
                format!("T-9 越界改动自动还原失败：{detail}")
            );
        }
    }

    #[test]
    fn scope_handled_event_names_session_and_count() {
        for patches in [json!([]), json!(["a"]), json!(["a", "b"])] {
            assert_eq!(
                event_line(
                    "session.scope_patches_handled",
                    "T-9",
                    &json!({"session_id":"S-12", "patches":patches})
                )
                .unwrap(),
                format!(
                    "T-9 越界补丁已交给会话 S-12 处理（{} 份）",
                    patches.as_array().unwrap().len()
                )
            );
        }
    }

    #[test]
    fn scope_task_detail_distinguishes_pending_and_handled() {
        assert!(scope_status_line(&json!({"last":null,"pending":[]})).is_none());
        let mut p = payload();
        p["at"] = json!("2026-11-13T00:00:00Z");
        for (pending, expected) in [
            (json!(["/tmp/b.patch"]), "待处理"),
            (json!([]), "已处理"),
            (json!(["other.patch"]), "已处理"),
        ] {
            let line = scope_status_line(&json!({"last":p,"pending":pending})).unwrap();
            assert_eq!(
                line,
                format!(
                    "  越界还原：{} 审计轮 #3 还原 4 个文件，补丁 /tmp/a.patch 等 2 份（{expected}）",
                    local_time("2026-11-13T00:00:00Z")
                )
            );
        }
    }
    #[test]
    fn test_fix_limit_status_explains_recovery() {
        assert_eq!(
            failure(&json!({"kind":"test_fix_limit","limit":2})),
            autome_domain::bug_loop::test_fix_limit_message(2)
        );
        assert_eq!(next_step(&json!({"id":"T-1","state":{"state":"failed","reason":{"kind":"test_fix_limit","limit":2}}})).as_deref(), Some("autome rerun T-1 locate    或取消后重提"));
    }
}

pub fn task_state_line(task: &Value) -> String {
    let line = state_line(&task["state"]);
    if task["test_fix"]["open"] == true && task["state"]["node"] == "locate" {
        line.replace(
            "定位",
            &format!(
                "定位（修正回归测试 {}/{}）",
                task["test_fix"]["used"], task["test_fix"]["limit"]
            ),
        )
    } else {
        line
    }
}

#[test]
fn test_fix_events_and_status_read_plainly() {
    use serde_json::json;
    assert!(
        event_line(
            "bug.test_fix_requested",
            "T-1",
            &json!({"attempt":1,"limit":2})
        )
        .unwrap()
        .contains("第 1 / 2 次")
    );
    for (result, expected) in [("accepted", "接受"), ("needs_fix", "回到修复轮")] {
        assert!(
            event_line("bug.test_fix_settled", "T-1", &json!({"result":result}))
                .unwrap()
                .contains(expected)
        );
    }
    assert_eq!(
        task_state_line(
            &json!({"state":{"state":"active","node":"locate"},"test_fix":{"used":1,"limit":2,"open":true}})
        ),
        "运行中 · 定位（修正回归测试 1/2）"
    );
}

#[test]
fn operator_note_status_lists_notes() {
    let task =
        serde_json::json!({"id":"T-1","state":{"state":"failed","reason":{"kind":"fix_budget"}}});
    assert!(
        note_budget_hint(&task, &serde_json::json!([{ "delivered":null }]))
            .unwrap()
            .contains("有未送达的备注")
    );
    assert!(note_budget_hint(&task, &serde_json::json!([{ "delivered":{} }])).is_none());
    assert!(note_budget_hint(&task, &serde_json::json!([])).is_none());

    let lines = note_status_lines(&serde_json::json!([
        {"n":1,"text":"原文\n下一行","at":"now","delivered":null},
        {"n":2,"text":"ok","at":"then","delivered":{"role":"impl","round":2}}
    ]));
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("未送达") && lines[0].contains("原文\n下一行"));
    assert!(lines[1].contains("已送达 impl 第 2 轮"));
}

#[test]
fn operator_note_events_render_one_line_each() {
    for kind in ["task.note_added", "task.note_delivered"] {
        let line = event_line(
            kind,
            "T-1",
            &serde_json::json!({"n":1,"text":"a\nb","notes":[1,2],"role":"impl","round":3}),
        )
        .unwrap();
        assert_eq!(line.lines().count(), 1);
        assert!(line.contains("T-1") && line.contains("备注"));
    }
}
