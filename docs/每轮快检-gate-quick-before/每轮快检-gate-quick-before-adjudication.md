# 每轮快检 裁决记录

## 第 1 轮裁决（design-round 0 → 1）

评审结论（`每轮快检-gate-quick-before-review.md`，基线 `eff6503`）：2 项阻塞意见（R1-01、R1-02），2 项非阻塞建议（N1、N2）。
本轮核对了评审引用的源码（`scheduler.rs::record_gate_result`、`protocol/prompts/fix-review.md`）后逐条裁决；两项阻塞意见均采纳并已改设计，
改后没有剩余的六类问题，设计定稿（`status: 设计中 → 实现中`，`design-round: 1/15`）。本轮无复提，没有争议项。

| ID | 主张摘要 | 设计位置 | 裁决 | 证据或理由 | 修改落点 | 复提计数 |
|---|---|---|---|---|---|---|
| D1-P01（R1-01） | §6 的熔断隔断条件「两条结果 `kind` 都有且不同」同样覆盖 `full ↔ repro`，改变了没配 `quick` 的项目（Bug `fix_gate` 在 `each_round` / `before_merge` 间切换）的熔断计数，违反约束 1 与 7 | 方案 §6 第一条；M-02 熔断用例 | **采纳**（违反任务目标） | `record_gate_result` 只按节点取上一条结果、用 `fingerprint(log, exit_code, timed_out)` 累计，不看命令或模式；复现命令先失败时 FixGate 在两种模式下的日志与退出码相同，反例成立。`full ↔ repro` 不是快检带来的变化 | 方案 §6：隔断条件改为「两者 `kind` 不同且至少一个是 `quick`」，`full ↔ repro` 与无 `kind` 的旧结果都照今天累计；M-02 熔断用例加「没配 `quick`：`full` 失败两次切 `repro` 第三次仍熔断，反向同理」对照 | 0 |
| D1-P02（R1-02） | 「实际跑了什么」的说明被「当前配置选的分段」限制：(1) 快检 FixGate 在途切 `each_round`，复核丢失说明；(2) `each_round` 的完整 FixGate 在途切到 `before_merge` + `quick`，复核被说成「只跑了复现命令」；(3) 需求快检在途切 `each_round`，审计简报仍写「全量门禁只在合并前运行」；另 Bug 快检说明缺退出码 | 方案 §7、§8（简报、复核、旧模板）、§9；M-04 | **采纳**（违反任务目标 / 内部逻辑矛盾：§8 自称「中途切换也不会说错」却做不到） | 核对 `protocol/prompts/fix-review.md`：只有 `before_merge` 段含宿主门禁说明，`each_round` 段没有；`launcher.rs` 的 `BUG_REVIEW_GATE_NOTE` 是固定句子，三条路径均可由设计文字推出。没配 `quick` 的项目这些路径的既有说法不动（约束 1） | 方案 §8「Bug 复核」：说明按最后一次 FixGate 的 `kind` / `command` / `exit_code` 生成（`quick`、`quick` 当前生效下的 `full`、其它三种），并把旧模板的补充句泛化为「最后一次 FixGate 是快检而正文没有 `{fix_gate_note}` 的位置时末尾追加一次」（含 `each_round` 段）；§8 审计简报快检文字的末句在当前为 `each_round` 时换成「已改为每轮运行完整门禁，这条结果是切换前的快检」；§7 表补一行并更新两行；M-04 复核 / 简报用例补三条路径 | 0 |
| D1-N1 | `full` 在途切到 `quick` 配置时，`command` 的来源要写明，避免 `kind: full, command: <quick>` | 方案 §3「收结果」 | **部分采纳**（非阻塞，写成澄清） | 「今天的逻辑」指不经过快检选择的原函数，`command` 不会记成快检命令；Bug FixGate 由 `each_round` 切 `before_merge` 时记成复现命令是 T-61 既有记账（有测试钉住），约束 1 要求保持 | 方案 §3 加一句澄清；Backlog B-03 | 0 |
| D1-N2 | 热缓存耗时「未取证」不等于测量完成，需明确补证责任 | 方案 §12、M-05 | **采纳为 Backlog**（非阻塞） | 环境受限不应阻塞里程碑（协议第 4 条）；不属于六类问题 | Backlog B-04 | 0 |

### 本轮对设计的修改清单

- 状态块：`status: 实现中`，`design-round: 1/15`；里程碑表 M-01…M-05 已完整（本轮未改）。
- §3 收结果：澄清 `command` 来源。§6：熔断隔断条件收窄到涉及 `quick`。§7：表中加一行、改两行。§8：简报末句按当前模式、复核说明按最后一次
  FixGate 的 `kind`、更正句机制泛化。M-02 / M-04 的新增用例描述同步。Backlog 加 B-03、B-04。
- 未改：任务文件、任务卡、`protocol/` 副本、`.autome/` 与 `kernel-contract` 区域。

提交由本轮 `git commit` 完成；若被拒绝则由 Autome 兜底。
