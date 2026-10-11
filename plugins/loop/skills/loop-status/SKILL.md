---
name: loop-status
description: 查看 Loop 运行状态——kill switch、人工卡点、各任务设计文档的状态块与里程碑计数、最近的运行记录与会话台账、正在运行的会话。用户问 loop 任务进展、卡在哪、跑到第几轮时使用。
argument-hint: "[slug]"
---

# 查看 Loop 状态

**参数**：用户调用本 skill 时附带的文本，可为空。非空时取第一个词作为 slug，只报告该任务；为空时报告全部任务。

收集以下信息（缺失项如实说明）：

1. **kill switch**：`.autome/SCHEDULE.json` 的 `autorun` 值。
2. **卡点**：`.autome/output/gates/` 下的 `<slug>.mode`（gated/auto）与 `<slug>.parked`（停靠中的启动 prompt；存在即需要 `loop-run <slug> approve` 放行或 `reject` 驳回）。
3. **任务清单**：`docs/*/*-task.md`。
4. **每个任务的进度**：读设计文档 `docs/<slug>/<slug>.md` 头部状态块——`status`、`design-round`、`implementation-round`、`current-milestone`、`current-milestone-reopens`、`convergence-mode`、`next-action`；统计里程碑三态（开放/待审/已完成）与各里程碑 `reopen-count`；看「争议项」小节有无冻结项。
5. **运行记录**：`docs/<slug>/retro.md` 最后几行。
6. **会话台账**：`.autome/output/sessions/launches.jsonl` 最后几条（时间、runtime、prompt 摘要）。
7. **运行中的会话**：

   ```bash
   ps aux | grep -E "claude -p|codex exec|cursor-agent -p|agent -p" | grep -v grep || echo "无运行中的 loop 会话"
   ```

报告格式：先一句话总态（如「实现循环进行中，implementation-round 3/15，M-02 待审，convergence-mode normal」），再分任务列关键数字，最后给建议动作（等待 / 放行卡点 / 仲裁争议项 / 人工终检 retro / 恢复 autorun 等）。
