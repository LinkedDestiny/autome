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

#[cfg(test)]
mod tests {
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
