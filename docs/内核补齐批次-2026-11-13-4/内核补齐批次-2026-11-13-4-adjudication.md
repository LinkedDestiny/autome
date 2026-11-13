# 内核补齐批次：裁决记录

## 裁决 #1（design-round 0 → 1）

评审基线：评审 #1（`内核补齐批次-2026-11-13-4-review.md`，基线 `0a57ce8e`）。本轮没有此前裁决，所有复提计数从 0 起。

评审结论：9 条阻塞问题。逐条回到源码核对（`scheduler.rs` 的 `regression_protection` / `reap_session` / `record_usage`、`git.rs` 的 `commit_paths`、
`integration.rs` 的 `integration_for_audit`、任务卡 A / D / G / H / I / J 各条），**9 条全部成立**，全部采纳并改了设计。2 条非阻塞建议（NB-01、NB-02）进 Backlog。
改动后没有剩余的六类问题，设计定稿：`status` 改为 `实现中`，`design-round` 1/15，里程碑表 16 行不变。

| ID | 主张摘要 | 设计位置 | 裁决 | 证据或理由 | 修改落点 | 复提计数 |
|---|---|---|---|---|---|---|
| D1-P01 | 修测试守卫把「复现命令点名的脚本」当可写测试，产品入口被点名后修测试轮可改它，违反 A-N3 / A-V2 | A 一节·修测试模式定位轮的守卫 | 采纳 | `regression_protection` 的 `script` 分支只看命令是否点名，用途是扩大保护；反例（`python app.py --self-check && python tests/repro.py`）成立 | 设计 A 一节：守卫允许集改为四类（任务文档、上一快照整文件路径、`regression_base` 处不存在的新测试文件与 `support`、内联测试段）；M-05 契约加 e2e `…locate_editing_a_command_named_product_script_is_rejected` 与对应负向对照，automed 11 → 13 | 0 |
| D1-P02 | 基线只覆盖测试路径，夹具缺失导致的退出 1 被当作有效复现 | A 一节·成对复现检查 | 采纳 | `bug_loop.rs` 判定只排除超时与 0/126/127；`regression_protection` 不识别 `fixtures/case.json`；反例成立 | 设计 A 一节：快照新增 `support` 集合（定位阶段写下的非测试命名支持文件，此时分支上尚无产品修复）随基线覆盖；覆盖后做「依赖完整性检查」，缺依赖按 `NotReproduced`（宁可让人看见）；残余的运行期未记录依赖划入 B-04 同类，不引入多语言编译器分析；M-05 加库用例 `test_fix_overlay_brings_support_files_and_flags_missing_dependencies` | 0 |
| D1-P03 | 保护集候选路径只来自「previous 快照 ∪ 本会话改动」，第一次定位未成功时没有快照，C-T1 原事故复现 | A 一节·重新快照；C 一节 | 采纳 | `scheduler.rs` 仅在 `Located` 时生成快照；反例（首次「无法复现」、第二次只改文档）按原公式得空集 | 设计 A 一节：`regression_set(base, HEAD, previous, session)` 增加第三来源「`base..本会话 locate_heads` 的累计改动」，不依赖更早会话是否有快照；命令点名脚本在累计来源里只认 `base` 处不存在的；C 一节引用同一函数；M-07 首条用例写明场景 | 0 |
| D1-P04 | 设计主动拒绝合并收尾节点上的备注，缩减 D-R1「任何未结束状态可留」与 D-R3 | D 一节·合并卡点；R7；B-05 | 采纳（部分采纳：合并已开始后的节点收下并警告） | D-R1 明文只拒已完成 / 已取消；`Rebase` / `MergeGate` 之后仍有收件轮可用。`Merging` / `MainCheck` / `Cleanup` 之后合并不可撤销、没有收件轮，无法送达，收下并以 `warning` + `late: true` 告知比拒绝更接近 D-R1 | 设计 D 一节与 X-4a：新增「合并前补轮边」（`Rebase` / `MergeGate` 通过、进入 `AwaitMerge` / `Merging` 前有未送达备注 → 需求 `start_impl`、Bug `start_fix`，`merge.auto` 同样适用）；M-08 加库用例、M-09 加 e2e `…note_during_merge_gate_adds_an_impl_round_before_merge`；R7 / B-05 改写 | 0 |
| D1-P05 | 审计简报窗口取「上一次审计会话之后」，漏处理被打回后备注在重审时消失，违反 D-R6 | D 一节·审计核对 | 采纳 | 反例（实现 #1 漏处理被审计 #1 打回，审计 #2 窗口排除该备注）成立 | 设计 D 一节：窗口起点 = 最近一次使里程碑关闭的审计会话，未得到审计通过的备注持续列出；交接审计、Bug 复核同口径；M-09 加 e2e `…unhandled_note_stays_in_the_audit_brief_after_a_reject` | 0 |
| D1-P06 | 真正旧协议任务还原指针后，`integration_for_audit` 因模板缺标记返回 `None`，进不了集成复验，与 J-E1 矛盾 | J 一节；背景·T-68 机制 | 采纳 | `integration.rs` 开头对缺 `<!-- integration-audit -->` 的模板直接返回 `None`，`scheduler` 传入的正是冻结模板；J-E1 要求「照常进入集成复验审计」 | 设计 J 一节新增「旧模板的集成复验」：缺标记但 `integration.range` 已记录时返回 `legacy: true` 载荷，简报由内核写集成复验一节，集成守卫对 `legacy` 降为 warn（J-E3），v8 行为不变；守卫表改行；M-10 加库用例并要求 e2e 用真实 v7 `audit.md`，automed 9 → 11 | 0 |
| D1-P07 | Onboarding 的 NULL `task_id` 映射伪键后，`reap_session` / `record_usage` / `session_log` 仍先查真实任务，收割与日志取不到，G-V2 不成立 | G 一节·Onboarding 会话进账本；R3 | 采纳 | `reap_session` 第一行 `get_task(&s.task_id)`，Onboarding 分支在其后；`record_usage` 取 `TaskRecord` 与任务文档；`session_log` 先遍历任务 | 设计 G 一节：`reap_session` 先按 kind 分流到 `reap_onboarding`、不走 `record_usage`；`session_log` 在 `task_id` 为空时按 `project_id` 取项目根；实现轮用 `rg` 列全 `get_task(&s.task_id)` 调用点并写进证据；R3 删去「只限 store 读写」的错误边界。用例名与数量不变（`…is_reaped_with_session_ended`、`…log_is_readable` 钉住） | 0 |
| D1-P08 | 失败夹具用 `pre-commit` 钩子，但 `init_commit` 走 `git commit --no-verify`，钩子不执行，用例观察不到失败 | H 一节·回归测试；M-15 | 采纳 | `git.rs` 的 `commit_paths` 明确 `commit --no-verify --no-gpg-sign` | 设计 H 一节与 M-15：故障改为在目标仓库预建 `.git/index.lock`（单仓库与工作区成员同法），清除后重试成功；用例改名 `workspace_onboarding_commit_single_repo_index_lock_keeps_state`；提交行为不改 | 0 |
| D1-P09 | 无条件迁移把可验证旧副本里升级前的真实修改写成新基线并永久吞掉，违反 I-E2 / I-E3 | I 一节·存量任务；M-16 | 采纳 | 旧 `current()` 以 `seed().hash()` 形成版本身份，同 tag 的旧任务其原始内容可由本二进制 seed 还原，不是「无法验证」 | 设计 I 一节区分三类：新冻结（`frozen`）、可验证旧任务（ref 的 tag / 哈希与本二进制 seed 吻合 → 改写为可信预期哈希而非副本现算哈希，真实差异照常告警）、无法验证旧任务（才重建基线，`verified: false` 并注明证据限制）；M-16 加两条 e2e，6 → 8 | 0 |

## 非阻塞建议的处置

- NB-01（F：`absolute` 指向主检出文件而内容来自默认分支）：进 Backlog B-07，面板以 `status_block` 与 `source` 为准，本轮不改设计。
- NB-02（H：`.autome/`、`AGENTS.md` 在工作区根、不在文档成员）：设计已按源码事实处理（工作区根文件不提交）；落点写成 Backlog B-06，搬迁产物不在本任务范围。

## 其它

- 里程碑表、依赖顺序与 16 个里程碑的划分不变；受影响的验收契约：M-05（13 个）、M-07（说明）、M-08（库 5）、M-09（19 个）、M-10（11 个）、M-15（用例改名）、M-16（8 个）。
- 状态块：`status: 实现中`、`design-round: 1/15`、`implementation-round: 0/60`（原为 0/0，16 个里程碑含返工的预估上限）、`current-milestone: M-01`。
- 没有争议项：全部主张首次提出，复提计数均为 0。
- 提交由 Autome 兜底（若本轮提交被拒绝）。
