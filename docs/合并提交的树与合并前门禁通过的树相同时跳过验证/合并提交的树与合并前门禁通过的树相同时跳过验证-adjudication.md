# 裁决记录

## 设计轮 #1 裁决

- 轮次：设计轮 1（`design-round` 0 → 1）。
- 评审基线：`f4aec03`（设计评审 #1）。
- 评审结论：3 条属于六类的问题，全部成立，已在设计里处理；1 条非阻塞建议进 Backlog。处理后没有剩余的六类问题，设计定稿，`status` 改为 `实现中`。
- 取证：裁决轮重读了 `scheduler.rs::run_gate`（3127–3423 行）、`record_gate_result`、`task.rs` 的 `(Node::Rebase, Ok)`、`store.rs` 的 `last_event_in`/`any_event_after`，并实测了 `pre-merge-commit` 钩子能否改动合并提交的树。

### D1-P01 · 结果事件中的命令不一定是实际执行的命令

- 主张：`gate.result.command` 取收集结果那个 tick 的配置，不是作业启动时的命令；A 在跑、配置改成 B 后，结果里记 B，设计据此误跳 B。
- 设计位置：方案 §2 第 3–4 步及末尾论证。
- 裁决：**采纳**。
- 证据：`scheduler.rs:3298–3318` 启动作业并写 `gate.started.command = gate.command`（启动 tick 的值）；`:3339–3346` 写 `gate.result.command = gate.command`，该 `gate` 来自本 tick 的 `resolve_config`（`:3134`）。两个 tick 之间改配置，两者就不同。反例成立。
- 修改：方案 §2 第 5 步重写——命令以 `R` 之前最近一条 `gate.started` 的 `node`/`sha`/`command` 为准（`R.command` 不再参与）；`store.rs` 新增只读 `last_event_before`；决策表改为按启动命令判（S5、S6），新增 S10、S11；新增 run_gate 层的真实用例 `main_check_reuse_ignores_a_result_whose_command_changed_mid_run`（配置在作业运行期间由 A 改为 B），并写明手写一条 `command: B` 的结果不算验证；M-02 负向对照补 (h)(i)。
- 复提计数：0。

### D1-P02 · 历史最新 MergeGate 不等于合并前最后一轮

- 主张：Rebase 完成时已移除 `[gate]`（直接绕过 MergeGate）或进入 MergeGate 后才移除，都会走完一轮而不写通过事件，旧一轮的 `P` 留在账本里，恢复配置后被误当作最后一轮。
- 设计位置：方案 §2 第 1–2 步及末尾论证。
- 裁决：**采纳问题，修法与评审的建议不同**。
- 证据：`task.rs:1262–1276` 在 `config.gate` 为空时直接转 AwaitMerge/Merging；`scheduler.rs:3172–3176` 无门禁时按通过结束、不写事件；`scheduler.rs:3108–3114` 说明 `merge.rebased` 只在要求合并时才写，账本里没有每一轮的起止标记，「把 P 绑到最后一轮的事件范围」做不到；要加标记事件又会破坏约束 2「逐事件相同」。
- 修改：方案 §2 新增第 4 步——合并提交的第二父提交（`git::second_parent`，M-01 新增）必须等于 `R.sha`，被合并的就是通过的那个提交；rebase 改写过 sha 的旧一轮记录自然对不上，sha 没变则那次通过正是对被合并提交做的。末尾论证改写；决策表新增 S7c；M-01 新增 `second_parent_*` 用例，计数相应调整（见下）；风险补第 6 条（比「树相同」窄一格，有意为之）。
- 复提计数：0。

### D1-P03 · E2 替换了验收 2 指定的 main 前进场景

- 主张：E2 改为在任务分支晚提交，没覆盖「main 在检查之后前进」，无法识别拿第二父提交的树代替合并提交的树的错误实现。
- 设计位置：方案 §7 `merge_after_a_late_branch_commit`、E2、对照表。
- 裁决：**部分采纳**。
- 理由与证据：(1) 评审指出的缺口成立——在新增第 4 步之后更明显：E2 与 R2–R4 里第二父提交本身就变了，树比较只由「main 前进」这一侧守。(2) 「必须是 e2e」不采纳：e2e 里 main 前进必走 `Behind` → 自动 rebase（现有用例 `main_moving_after_the_press_takes_the_same_automatic_path` 守着），竞争窗口无法稳定构造；实测 `pre-merge-commit` 钩子里改索引不进合并提交（合并提交树里没有钩子加的文件），行不通。任务卡验收 2 的括号写明「走自动 rebase 之外的构造」，late-branch 属此类，没有偏离。
- 修改：保留 E2；新增 run_gate 层真实 Git 用例 `main_check_reuse_when_main_advanced_after_the_check_runs_the_main_check`（绕过 `check_merge` 手工 `git merge --no-ff`，使合并提交的树 ≠ 第二父提交的树）与决策表 S7a/S7b；M-02 负向对照 (c) 改为这两处失败，并加入「比第二父提交的树」的错误实现；对照表验收 2 一行改写；§7 写明 E2 不守这一侧及理由。
- 复提计数：0。

### D1-N1（非阻塞）· M-03 的 `grep -c` 对多文件退出码不能代表每个文件

- 裁决：进 Backlog（`D1-B01`），不触发轮次。

## 计数更新（因 D1-P02 增加 `git::second_parent`、D1-P01/P03 增加 2 个 scheduler 用例）

| 节点 | 旧 | 新 |
|---|---|---|
| M-01 `git::` / 全工作区 | 37 / 1215 | 38 / 1216 |
| M-02 `scheduler::` / 全工作区 | 89 / 1220 | 91 / 1223 |
| M-03 全工作区 | 1221 | 1224 |

## 定稿判断

没有剩余的六类问题：关键事实（命令来源）已按源码修正；轮次认定改用提交绑定，不依赖账本里不存在的标记；验收 2 两侧都有确定性守卫；里程碑顺序、验收命令与计数已同步。`status` 改为 `实现中`，`current-milestone` 置 `M-01`，等用户批准。

本轮改动已由 `git commit` 正常提交，不需要 Autome 兜底。
