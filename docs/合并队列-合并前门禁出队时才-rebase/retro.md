设计 #1 | 无 | 设计初稿（5 个里程碑，待评审） | docs/合并队列-合并前门禁出队时才-rebase/合并队列-合并前门禁出队时才-rebase.md | 无
实现 #1 | M-01 | 待审 | docs/合并队列-合并前门禁出队时才-rebase/evidence/M-01-r1-impl.md | 无
实现 #2 | M-01 | 待审 | docs/合并队列-合并前门禁出队时才-rebase/evidence/M-01-r2-impl.md | 无
审计 #1 | M-01 | 通过 | docs/合并队列-合并前门禁出队时才-rebase/evidence/M-01-r2-audit.md | 无
实现 #3 | M-02 | 待审 | docs/合并队列-合并前门禁出队时才-rebase/evidence/M-02-r3-impl.md | 无
审计 #2 | M-02 | 通过 | docs/合并队列-合并前门禁出队时才-rebase/evidence/M-02-r3-audit.md | 无
实现 #4 | M-03 | 待审 | docs/合并队列-合并前门禁出队时才-rebase/evidence/M-03-r4-impl.md | 无
审计 #3 | M-03 | 通过 | docs/合并队列-合并前门禁出队时才-rebase/evidence/M-03-r4-audit.md | 无
实现 #5 | M-04 | 待审 | docs/合并队列-合并前门禁出队时才-rebase/evidence/M-04-r5-impl.md | 无
审计 #4 | M-04 | 通过 | docs/合并队列-合并前门禁出队时才-rebase/evidence/M-04-r5-audit.md | 无
实现 #6 | M-05 | 待审 | docs/合并队列-合并前门禁出队时才-rebase/evidence/M-05-r6-impl.md | 无
审计 #5 | M-05 | 通过 | docs/合并队列-合并前门禁出队时才-rebase/evidence/M-05-r6-audit.md | 无
复盘 #1 | 无 | 教训 4 条 | docs/合并队列-合并前门禁出队时才-rebase/lessons.md | 无

## 终止总结

- 终止状态：已完成。M-01 到 M-05 都由审计关闭，设计状态块写的是 `status: 已完成`。
- 轮数：设计 1/15（评审 1 轮提了 3 个阻塞问题，裁决全部采纳后定稿，没有再评审），实现 6/25，审计 5 轮。
- 各里程碑：M-01 在审计 #1 关闭（用了实现 #1、#2；#2 修宿主快检报的 Clippy），M-02 在审计 #2 关闭，M-03 在审计 #3 关闭，M-04 在审计 #4 关闭，M-05 在审计 #5 关闭。全部 reopen 0 次，没有重复出问题的领域；收敛模式一直是 normal。
- 实现缺陷 0 个，验证缺口 0 个（以审计证据为准）。另外：设计「验证安排」要求每个实现轮自己跑 fmt 和 clippy，但实现轮证据里都没写这两条的退出码，见 lessons L-01。
- 预算：本任务没有预算行，不适用。
- 未完成部分：无。
- 人工验收清单还有两条没确认：H-01（装好的 `autome` 上 status/ls 的过期提示排版）、H-02（Web 面板和桌面端显示 `merge` 字段与新事件）。
- 协议改进建议（1 条）：实现轮提示词里“不要自己跑项目门禁命令”和设计里点名给实现轮跑的 fmt/clippy 冲突了。实现轮按提示词没跑，宿主快检在 M-01 抓到 `clippy::nonminimal_bool`，多花了一轮实现。证据：`evidence/M-01-r1-impl.md`（写了“没有运行”）、`evidence/M-01-r2-impl.md`（Clippy 先红后绿）。建议在提示词里说清楚：设计点名给实现轮的检查命令不算被禁止的门禁命令。
