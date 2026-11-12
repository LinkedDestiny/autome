# 设计裁决记录

## 裁决 #1（design-round 0 → 1）

评审依据：`门禁只在合并前跑一次-gate-run-review.md` 评审 #1（基线 `e6e260f`）。本轮逐条核对了评审引用的源码
（`scheduler.rs::run_gate`、`gate.rs::Job / start / discard_node`、`launcher.rs::PromptSpec / build_prompt`），三条主张的事实都成立。

评审结论：3 条六类范围内的问题，**全部采纳**；之前没有裁决，复提计数均为 0。采纳后设计已修改，本轮认为没有剩余的六类问题，
状态块转 `实现中`，里程碑表见设计文档。

### D1-P01 · 启动记录缺失时回退当前命令，仍会把复现通过当成完整门禁通过

- 主张摘要：方案 §5 让 FixGate 结果的 `command` 取作业的 `gate.started` 事件，找不到才用当前配置。`gate::start` 与 `gate.started`
  两步不原子（`scheduler.rs` 先 `gate::start` 再 `append_event`），daemon 在两步之间退出后，`before_merge → each_round`
  切换会把一个只跑了复现命令的作业记成完整命令，同 head 的 MergeGate 因此沿用。
- 设计位置：方案 §5 第三条「Bug 任务，FixGate 正在跑时切换 `run`」；M-03。
- 裁决：**采纳**。
- 证据：`run_gate` 在 `gate::start`（约 3298 行）之后才写 `gate.started`（约 3309 行）；`gate.rs::start` 只往 `.started` 文件写时间戳，
  `poll` 以它解析超时，文件里放不下命令；沿用判断（约 3212–3216 行）确实只比较 `ok`、`sha`、`command`。反例成立。
  同根情形（同 node/head 旧作业被丢弃后重启，新启动事件写入前崩溃，按 node/sha 取事件会拿到上一次运行的命令）也成立——
  所以「从事件里找」这条路本身不可靠，不是补一个兜底就能修。
- 修改落点：§5 第三条整段重写——不再依赖账本事件，改用**作业旁的标记文件** `<node>-<head12>.repro-only`：`run_gate` 在
  `gate::start` 之前确定性地写入或删除它（只有「Bug 任务、节点 FixGate、有项目门禁、`gate_run == BeforeMerge`」才写），
  `Job::clear` 与 `discard_node` 清理它。FixGate 结果记账时，标记在 → `command` 取复现命令本身；不在 → 取当前配置命令（今天的行为）。
  标记早于作业出现，故任何以 `before_merge` 启动的 FixGate 作业都带标记，崩溃窗口不存在。§10 补两条否决项；M-03 加崩溃窗口用例。
- 复提计数：0。

### D1-P02 · 明确排除 Bug 复核提示，遗漏任务卡要求的一个交付对象

- 主张摘要：任务卡要求「审计 / 复核简报」都写明没有本轮宿主门禁结果；设计只做了审计简报，并把复核轮明确排除。
- 设计位置：非目标「不给 Bug 复核轮新增简报」；方案 §6.3；M-04。
- 裁决：**采纳**，做法与评审建议的两个选项都不同：不新增简报文件，在复核轮 **prompt** 末尾由宿主注入一段说明。
- 证据与理由：任务文件 §2 第 4 条与卡原文都写「审计 / 复核简报」；约束 8 要求「写清复核简报怎样处理」，设计原先的处理是「不处理」，
  而卡的「不留空」要的是交付。但 Bug 会话没有简报，复核 prompt（`protocol/prompts/fix-review.md`）也不指向任何简报文件，
  新增简报要改 `protocol/`（范围外）。宿主往 prompt 里追加文字是现成的通道（`Inject` 的 `GateFailed` 就是这么给修复轮的），
  不碰协议源、不新增事件或方法。
- 修改落点：§6.3 重写；新增 `PromptSpec.gate_note: Option<&str>`（`launcher::build_prompt` 在 inject 段之后追加）和
  `brief::bug_review_gate_note(&ResolvedConfig)`（有项目门禁且 `BeforeMerge` 才返回，文案含「本项目门禁只在合并前运行，本轮没有宿主门禁结果」
  并写明「修复门禁只跑了复现命令，通过不等于项目门禁通过」）；调度器只对 `Bug{FixReview}` 传入。`PromptSpec` 的字面量有 4 处
  （`scheduler.rs` 2 处、`launcher.rs` 测试 1 处、`prompt_rendering.rs` 辅助函数 1 处）要补 `gate_note: None`，属于约束 1 允许的
  「结构体多字段导致构造处编译不过」，逐条列入 §9。金样：新增 `golden/prompts/fix-review-before-merge.md`；M-04 加 1 个 e2e
  断言复核轮 prompt 的实际接线。
- 复提计数：0。

### D1-P03 · 命令来源修正无条件作用于 FixGate，改变未启用新模式的行为

- 主张摘要：§5 原修法让所有 FixGate 的 `gate.result` / `GateOutcome::Failed.command` 取启动事件的命令；用户从不写 `run`、
  只在 FixGate 运行中改项目门禁命令 A→B 时，今天记 B，按设计记 A，约束 1 被破坏。
- 设计位置：方案 §5 第 202–204 行。
- 裁决：**采纳**。
- 证据：`scheduler.rs` 约 3344 行 `"command": gate.command` 与约 3418 行 `command: gate.command.clone()` 取当前 tick 的配置；
  约 3216 行 Bug 沿用比较该命令。评审的反例成立，原文「配置没变时逐字节不变」比约束 1 的「没有启用新模式」窄。
- 修改落点：随 D1-P01 一并解决——新机制只在作业带 `.repro-only` 标记时偏离今天的行为，而标记只可能由 `before_merge` 下的 FixGate 写出；
  从未出现 `before_merge` 的任务永远没有标记，记账路径与今天逐字节相同。§5 明写这一边界；M-03 加 `each_round` 下 A→B 的用例钉住。
- 复提计数：0。

### Backlog 处置

- N-01（「每个任务一次」的限定）：进 Backlog **B-03**，README / 技术设计写作时紧邻注明「正常通过路径上的合并前门禁；MainCheck 仍运行，MergeGate 失败会重试」。
  不触发轮次。

### 设计定稿判断

三条问题都已在设计里闭合，改动局限在 §5、§6.3、§9、§10 与 M-03 / M-04 / M-05 的用例清单；没有触及配置类型、需求状态机、MergeGate 回路等
评审认可的部分。逐类自检：关键事实（标记文件由 `run_gate` 写在 `gate::start` 之前，`Job::clear` / `discard_node` 清理；`PromptSpec`
共 4 处字面量）、内部逻辑（标记缺失 = 今天行为，作业早于标记出现不可能）、项目规则（`gate.rs` 只加一个旁路文件，不重构）、任务目标（约束 1、6、8 均有对应用例）、
里程碑可执行性（用例数与前置依赖已重算）、重大风险（崩溃窗口由「先标记后启动」消除）——没有剩余的六类问题。`status` 改为 `实现中`，`design-round` 为 1/15。

本轮改动在会话结束前提交到当前分支；若提交被拒，由 Autome 兜底。
