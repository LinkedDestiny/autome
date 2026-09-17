---
description: 查看 Loop 运行状态：kill switch、卡点、设计文档状态块、里程碑计数、运行中的会话
argument-hint: "[slug]"
allowed-tools: Bash, Read, Glob, Grep
---

汇总当前项目的 Loop 状态并用简洁中文报告。

**参数解析**：完整参数串为：`$ARGUMENTS`（可为空）。非空时取第 1 个词作为 slug，只报告该任务；为空时报告全部任务。

收集以下信息（缺失项如实说明）：

1. **kill switch**：`.autome/SCHEDULE.json` 的 `autorun` 值。
2. **卡点**：`.autome/output/gates/` 下的 `<slug>.mode`（gated/auto）与 `<slug>.parked`（停靠中的启动 prompt；存在即需人工 `autome approve <slug>` 放行）。
3. **任务清单**：`docs/*/*-task.md`。
4. **每个任务的进度**：读设计文档 `docs/<slug>/<slug>.md` 头部状态块——`status`、`design-round`、`implementation-round`、`current-milestone`、`current-milestone-reopens`、`convergence-mode`、`next-action`；统计里程碑三态计数（开放/待审/已完成）与各里程碑 `reopen-count`；查看「争议项」小节有无冻结项。
5. **运行记录**：`docs/<slug>/retro.md` 最后几行（最近轮次动向）。
6. **会话台账**：`.autome/output/sessions/launches.jsonl` 最后几条（时间、runtime、prompt 摘要）。
7. **运行中的会话**：

```bash
ps aux | grep -E "claude -p|codex exec" | grep -v grep || echo "无运行中的 loop 会话"
```

报告格式：先一句话总态（如「实现循环进行中，implementation-round 3/15，M-02 待审，convergence-mode normal」），再分任务列关键数字，最后给出建议动作（等待 / approve 放行 / 仲裁争议项 / 人工终检 retro / 恢复 autorun 等）。
