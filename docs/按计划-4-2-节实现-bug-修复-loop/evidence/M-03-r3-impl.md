# M-03 实现证据 · 实现 #3

范围：仅 M-03；实现轮数 3/30。起点 `ba8e181`。上一轮 M-02 审计通过，无退回缺陷、无须逐字移植的审计检查。

## 实现与修复说明

- 调度器按 `uses_bug_loop` 选表，旧 Bug 继续需求 Loop；新增定位、修复、复核会话及事件。
- Bug 文档独立读取，状态与复核字段交给已有领域守卫检查；缺文件、解析失败进入 Protocol 失败。Bug 会话不生成需求简报，定位引用已提交的卡片。
- 账本统计修复轮次与最近 reply 后的未复现次数；崩溃重试、Reenter、冷却沿用既有路径，重入不增加修复轮次。修复和复核的预算行均取账本。
- 三种 Bug 门禁复用原有进程、超时、日志机制。修复和合并前门禁使用已成功复现时记录的命令，组合执行复现检查后再执行项目 gate；文档改命令不改变宿主实际执行命令。
- 合并前复用仅对新 Bug Loop 增加命令一致条件；需求、旧 Bug 仍按通过和 SHA 复用。
- 8 个 e2e + 3 个调度器单测。Bug 文档替身从协议契约区提取；真实 shell 回归检查读取产品文件，修复只改产品文件。卡片旧测试按设计 §11 第 3、4 条改写；共用 settle 夹具增加两个 Bug 内核节点，避免门禁还在运行就提前结束测试。

## 验收命令与数量

最终命令和结果在全部缺陷注入还原后记录；专项与全量重叠用例不重复累计。

| 命令 | 结果 | 相比上一轮 |
|---|---|---|
| `cargo test -p automed --test end_to_end bug_loop` | 8 passed，0 failed/ignored | 0 → 8，新增 M-03 的 8 个场景 |
| `cargo test -p automed --lib scheduler::tests::bug_loop` | 3 passed，0 failed/ignored | 0 → 3，冷却隔离、重入、门禁复用 |
| `cargo test -p automed --test end_to_end task_card` | 3 passed，0 failed/ignored | 3 → 3，卡片契约未减少 |
| `cargo test -p autome-cli --test cli task_card_a_brief_piped_on_stdin_creates_a_bug_draft_that_starts_with_its_card` | 1 passed，0 failed/ignored | 1 → 1，保留卡片字节检查 |
| `cargo test -p automed --test end_to_end` | 67 passed，0 failed/ignored | 59 → 67，净增 8 |
| `cargo test -p autome-domain` | 345 passed，0 failed/ignored（doc-test 0） | 345 → 345 |
| `cargo test -p automed --lib` | 598 passed，0 failed/ignored | 595 → 598，净增 3 |
| `cargo clippy --all-targets -- -D warnings` | 退出 0 | 无新增警告 |
| `cargo fmt --all -- --check` | 退出 0 | 格式检查 |

输出分别为 `.autome/output/r3-final-{bug,unit,card,cli,e2e,domain,lib,clippy,fmt}.txt`。
本里程碑新增测试总数 **11 = 8 + 3**，与设计一致。本轮实际执行的领域、automed 单元、e2e 和指定 CLI 用例去重共 **1011**，对应上一轮基线 **1000，净增 11**。上轮 workspace 总数 1047；本轮未执行 workspace 全量，不作全量通过声明。CLI reply、工作区选择、全量兼容和回归文件保护仍分别由 M-04/M-05/M-06 推进。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计情形 | 检查及结果 |
|---|---|
| 定位为已定位 | 完整合并 e2e：locate → repro_check，宿主执行真实失败脚本 |
| 无法复现、不是 Bug | M-01 `cannot_reproduce_and_not_a_bug_wait_without_a_slot`；本轮卡片 e2e 实跑 await_info |
| 定位后复核必须待复核 | 领域守卫及 malformed 文档 e2e；解析缺字段、缺文件均失败 |
| 修复保持已定位、复核重置 | malformed e2e 的 changed-status / stale-review 拒绝 |
| 复核通过或打回 | happy/review_rejection e2e；pending-review 被拒绝 |
| 文档中的修复次数不作权威 | 替身保留契约的 0/3，账本仍记录第 1、2 轮；修复和复核 prompt 显示正确轮次 |
| 复现命令不随文档变化 | fix-failure e2e：把文档命令改 true，原回归仍失败并退回修复 |
| 退出 0、126、127、超时 | repro-miss e2e + 领域 timeout/unrunnable 用例；gate 超时单测实跑进程组终止 |
| 修复门禁先复现后项目 gate | 项目 gate e2e 检查两段输出顺序，项目失败仍退回修复 |
| 无项目 gate 也必须验证复现 | happy/fix-failure e2e |
| 合并前复用同 SHA 同命令 | scheduler 门禁复用测试：新 Bug 命令不同重跑，相同复用；需求和旧 Bug 命令不同仍复用 |
| 三类会话预算、模板、卡片、无简报 | happy、task_card、review_rejection、reenter 用例 |
| 冷却保持 SAME-MODEL | scheduler 冷却测试双向检查 Fix/FixReview 都等待，不改道到对方 runtime |
| 崩溃重试、重入不多计轮 | crash e2e 与三个 Bug 节点 reenter 单测 |
| 新 Bug 与 K1 旧 Bug 分流 | 旧 Bug e2e 从 await_design_approval 经 implement/audit/retro 到 Done |
| 工作区 repos | M-04 范围，本轮未实现或声称通过该项 |

### 2. 设计点名的失败分支

- 复现不失败：退出 0 回定位；命令不存在（127）、不可执行码（126）不算复现，检查注入的原因及日志；第二次未复现由 M-01 领域用例验证。
- 复现超时：领域用例拒绝 timeout；既有 gate 超时测试通过人为提前 started 时间，使真实 sleep 进程组被终止并返回 TIMED_OUT。不是用 sleep 等待超时预算。
- 修复复现失败、项目 gate 失败、复核打回：三个独立 e2e，各回到修复，正确增加账本轮次与 prompt 注入。
- 缺文件/读盘失败、空命令、非法状态、陈旧复核：malformed e2e 的五个子场景与领域解析器边界用例拒绝；不读取需求设计文档兜底。
- 崩溃：真实替身 exit 17，既有 retry 后同一修复轮继续，计数保持 1。
- 角色关闭、预算耗尽、rebase 冲突、merge_gate 失败：领域相应用例实跑；预算及用户 extend 的运行时 e2e 属 M-04。
- 外部授权拒绝、外部服务不可达：不适用。本里程碑不接入授权服务或网络服务；模型使用本地替身，模型异常沿用崩溃路径。

### 3. 全部状态迁移

本轮运行领域全量，含 Bug Loop 29 个用例；按设计 §1 转换表逐行核对：

| 迁移 | 证据 |
|---|---|
| Queued → Locate / Impl 关闭 → Config | queued、impl_disabled；happy e2e |
| Locate → ReproCheck / AwaitInfo / NotABug | located、cannot_reproduce_and_not_a_bug；happy/card e2e |
| ReproCheck 失败码 → Fix；0/超时/126/127 → Locate；第二次未复现 → AwaitInfo | reproduced、exit_zero、timeout_and_unrunnable、second_consecutive_miss；repro-miss e2e |
| Fix → FixGate（无配置也跑） | fix_always_runs；happy e2e |
| FixGate 成功 → FixReview；审计关闭 → Rebase；失败 → Fix | passed_fix_gate、failed_fix_gate；两个门禁失败 e2e |
| FixReview 通过 → Rebase；打回 → Fix | review_pass、review_reject；happy/rejection e2e |
| Rebase 成功 → MergeGate；冲突 → Fix | rebase_always、rebase_conflict；happy e2e |
| MergeGate 失败 → Fix；通过 → AwaitMerge | merge_gate_failure、merge_tail；happy/reuse |
| AwaitMerge → Merging → MainCheck/Cleanup → Done | merge_tail；完整合并 e2e |
| AwaitInfo/NotABug + Reply → Locate；空/非等待 Reply 拒绝 | reply_returns、reply_is_refused（RPC 留 M-04） |
| FixBudget + Extend → Fix | extend_resumes_fix（RPC 留 M-04） |
| Failed/Stopped + RerunFrom 四个合法节点，其它拒绝 | rerun_from_accepts_locate_fix_review_and_rebase_only |
| Stopped 的两个 Bug 门禁 + Resume → Requeued + Discard | stopped_bug_gates_discard_runs_and_requeued_bug_nodes_reenter |
| 取消、暂停、停止、恢复及 Requeued 出队 | 上一行、waiting_nodes_allow_cancel、feature_table_rejects_bug_triggers；调度器 reenter 检查三种会话种类 |
| 会话 MissingArtifact/Unparseable/GuardFailed/Crashed → 失败；可重试崩溃 → 原节点 | session_errors、malformed/crash e2e |

### 4. 输入域边界

- 领域解析器实跑：空串、缺键、重复键；命令和根因为空/“无”；0/0、01/03、u32 最大值、k 大于 N 可解析；负数、溢出、全角数字、分隔符错误拒绝。闰日/跨年不适用：Bug 状态块没有日期输入域。
- shell 退出边界：0 与非零；126/127 和 TIMED_OUT 单独拒绝为成功复现。
- 单条和两条修复事件、首次无 budget 的默认 3；reply 前后未复现计数 1 → 0（store 查询单测通过 scheduler 实跑）。
- 单仓库卡片多字节、CRLF、占位符字面文本沿用 task_card 断言；用户补充正文、工作区成员边界属 M-04。

## 负向对照和验证过程

| 注入错误 | 检查结果 |
|---|---|
| 对需求任务也要求复用命令相同 | scheduler 门禁复用测试红，退出 101 |
| 把退出 0 当作已复现 | repro-miss e2e 红，退出 101 |
| 修复门禁跳过复现，直接 true | fix-failure e2e 红，退出 101 |
| 崩溃重入增加 fix_round 事件 | crash e2e 红，退出 101 |
| 分流只看 kind | 原用例只检查批准后的 implement，错误地绿；加强到完整合并后红（误进 MergeGate，缺少复现命令），退出 101 |

日志 `.autome/output/r3-negative-{reuse,zero-repro,skip-repro,retry-count,legacy-final}.txt`。全部是运行断言失败，非编译失败。所有注入已还原；`autome-domain/src/bug_loop.rs` 无最终差异。旧 Bug 用例现在检查每次 tick 的 errors，故上述缺陷立即失败，无须等待 settle 的 90 秒上限。

早期开发过程中修正过测试代码的事件 tuple 解构、宏声明顺序，以及临时配置夹具（save_project 不保存 gate，改为直接写测试配置）。一次验收启动时负向子进程尚未结束，读取了注入版本；该次不能作为验收，最终在全部子进程退出、源码还原后重跑。

## 提交与结论

M-03 验收命令全部通过，自审完成，标为待审，交审计轮独立复验。实现轮未关闭里程碑；当前轮次 3/30。提交当前分支，不推送、不启动下一会话。
