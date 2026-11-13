# M-04 · 实现 #5

本轮仅推进 M-04。上一轮审计 #4 通过 M-03，没有退回缺陷，因此不存在须逐字搬入并先红后绿的审计检查。本轮新增设计指定的四个真实 Git / worktree / 包装脚本 / 调度器 e2e；未修改产品实现、既有测试行或金样。

## 验收命令与数量

| 命令 | 结果 | 总数与基线差 | 原始输出 |
|---|---|---|---|
| `cargo test -p automed --test end_to_end integration_e2e` | 4 通过，0 失败，0 忽略 | 4，相对基线 0 增加 4；即下表四个新增场景 | `.autome/output/m04-integration-final.txt` |
| `cargo test -p automed --test end_to_end before_merge` | 11 通过，0 失败，0 忽略 | 11，相对契约基线 11 差 0 | `.autome/output/m04-before-merge.txt` |
| `cargo test -p automed --test end_to_end a_gate_failing_after_the_automatic_rebase` | 1 通过，0 失败，0 忽略 | 1，相对基线 1 差 0 | `.autome/output/m04-auto-rebase.txt` |
| `cargo test -p automed --test end_to_end a_failing_gate_sends_the_work_back` | 1 通过，0 失败，0 忽略 | 1，相对基线 1 差 0 | `.autome/output/m04-gate-back.txt` |
| `! git diff -U0 "$(git merge-base HEAD main)" -- crates/automed/tests/end_to_end.rs \| grep -q '^-[^-]'` | 退出码 0 | 既有 e2e 没有删除或改写行 | 命令直接检查 |

四个测试命令均成功；本里程碑验收去重后共 **17 个用例，比本轮之前的相同筛选范围 13 个增加 4 个**。上一轮 M-03 跑的是 prompt_rendering 35 + integration_section 6，共 41 个；跨轮表面数量 17−41 = −24 来自换成 M-04 指定的 e2e 子集，不代表删测。新增前 e2e 整个二进制有 159 个用例，现在 163 个（4 个执行、159 个过滤）；过滤项未执行，不计为通过或忽略。

按本轮用户指令，未执行全量 `sh scripts/ci-local.sh`，也未执行宿主负责的 fmt/clippy/Web 快检。只用 `rustfmt --edition 2024 crates/automed/tests/end_to_end.rs` 格式化本轮文件，`git diff --check` 通过。提交使用 `NO_CI=1`，避免 post-commit 启动全量门禁。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计 §9 / M-04 情形 | 项目测试与实际判据 | 结果 |
|---|---|---|
| 全关闭后 rebase 冲突 → 修复 → 集成复验 → 合并卡点 | `integration_e2e_rebase_conflict_after_all_done_is_reviewed_then_reaches_await_merge`：真实 main 与任务分支修改同一行；脚本要求 rebase 先失败再解决；断言冲突文件 README.md、测试文件 tests/conflict.txt、集成 prompt/简报、唯一 integration.range，以及最终保留两侧语义的文件内容 | 通过 |
| 合并前门禁失败 → 修复 → 审计退回 → 普通实现/审计 | `integration_e2e_merge_gate_fix_reopened_by_the_audit_returns_to_implement`：断言门禁命令与失败尾部，reopen=1、milestone.contradicted=1、implementation-round=3/25；后续普通审计不再带集成段，最终 AwaitMerge | 通过 |
| 实现轮预写同 k 审计文件，审计只说无待审 | `integration_e2e_audit_without_evidence_stops_with_a_readable_reason`：复用 World::EVIDENCE 预写文件；审计不写；断言 Failed、audit_made_no_progress，中文 detail 含集成复验、实际起止 SHA 前 7 位、证据路径与开放出口，下一复盘未启动 | 通过 |
| 全关闭后的实现轮只有文档目录内提交 | `integration_e2e_doc_only_repair_keeps_todays_audit`：外部门禁条件恢复，修复会话只提交文档；断言无 integration.range、无 guard.failed、无集成 prompt/简报，空审计仍到 AwaitMerge | 通过 |
| 新路径以外既有行为 | before_merge 11 个及两个既有回退用例保持原文通过；零删除行检查 | 通过 |

### 2. 失败分支

- 真实 rebase 冲突：第一个新用例执行并解决，之后第二次 Rebase 干净退出，没有重复集成循环。
- 真实 MergeGate 非零退出：第二、第三、第四新用例均先失败，后分别验证退回、阻断、恢复。
- 集成审计无本轮证据（含实现轮预写同名文件）：第三个用例必须失败等人，不能静默放行。
- 拒绝授权、读盘失败、超时、空输入、外部服务不可达：M-04 未新增这些输入或失败处理分支，不适用；范围读取失败属于已关闭 M-01 契约，本轮不扩跑该子集。第四用例中的外部文件条件模拟门禁恢复，不声称验证了网络服务。

### 3. 全部迁移

- 冲突分支：普通 Audit → Retro → Rebase 冲突；事件断言精确后缀为 `implement, audit, retro, rebase, await_merge`。
- 门禁退回分支：MergeGate 失败后的精确后缀为 `implement, audit, implement, audit, retro, rebase, merge_gate, await_merge`。退回开放 → 待审 → 已完成，并保留 reopen=1；最终只有一个集成范围事件。
- 无证据分支：Implement → 集成 Audit → Failed，预备的下一轮复盘脚本没有启动。
- 空范围分支：MergeGate 失败 → 文档修复 Implement → 普通 Audit（无证据也合法）→ Retro → Rebase → MergeGate → AwaitMerge。
- 上述状态/事件均由真实调度执行；不涉及新增状态或多通道，不重复验收原有预算耗尽/角色关闭路径。

### 4. 输入域边界

- 目录外提交 0 个：doc-only 用例；目录外提交至少 1 个：其余三个用例。
- 全表仅 1 个里程碑且已完成：四个用例；退回后表非全完成：reopen 用例，后续不误判集成。
- 证据文件已存在但本审计未写：无证据用例；同一文件本审计重写：成功冲突/退回用例。
- rebase 改写历史与门禁线性提交两种范围均实际执行；测试路径独立列出、SHA 前 7 位用于可读错误的断言。
- 日期、前导零、最大数字、空串等不是此里程碑输入域，不适用。

## 调试与负向对照

首次新用例运行 3 通过、冲突用例失败（`.autome/output/m04-integration.txt`）；诊断复跑仍失败（`.autome/output/m04-conflict-retry.txt`）。替身包装器用测试二进制充当日志 renderer，其提前退出使 rebase 的输出管道发生 SIGPIPE（会话退出码 141），替身脚本随后错位。仅将新增测试脚本里的两次 rebase 输出重定向到 `/dev/null`，仍严格检查退出码：首次 rebase 必须非零、解决后 `--continue` 必须成功；没有吞掉失败或改生产逻辑。修正后四项均通过。

负向对照：临时把 `scheduler.rs` 的 `Round.integration: integration_audit.as_ref()` 改为 `None`，运行 `cargo test -p automed --test end_to_end integration_e2e_audit_without_evidence_stops_with_a_readable_reason`。退出码 101，1 个用例失败，断言实际 `active`、预期 `failed`（`.autome/output/m04-negative.txt` 与 `m04-negative.exit`）。这证明真实生产接线丢失会被用例抓住。随后恢复源码；`git diff --quiet -- crates/automed/src/scheduler.rs` 为 0。恢复后完整四项重跑结果见 `.autome/output/m04-restored.txt`；该次也包含给无证据测试预备的复盘步骤，确保错误放行会真正继续至合并卡点，而不会只因缺替身脚本失败。

恢复后四项重跑：退出码 0，4 通过 / 0 失败 / 0 忽略，总数相对本轮首次四项筛选不变。四项自审已执行完成后，M-04 标为待审；状态块 implementation-round 为 5/25，未关闭任何里程碑。
