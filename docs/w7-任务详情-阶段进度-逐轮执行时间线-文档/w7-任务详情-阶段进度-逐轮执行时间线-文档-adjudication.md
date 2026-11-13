# W7 设计裁决记录

## 第 1 轮（design-round 0 → 1）

评审结论：评审第 1 轮提出 4 项属于协议六类的问题（P-01..P-04）与 1 条非阻塞建议（S-01）。本轮逐条读源码复核，4 项全部成立，均已采纳并改写设计；
S-01 进 Backlog。复提计数均为 0（首次提出，无既有裁决）。

### D1-P01 门禁三元组不能关联排队事件，也不能区分同一提交上的多次执行

- **主张摘要：** 按 `(node, kind, sha)` 合并门禁事件并以 `gate:<node>:<kind>:<sha>` 为键：`gate.queued` 没有 `kind`/`sha`，同提交同因重跑三元组相同。
- **设计位置：** 「执行时间线」内核步骤条目的门禁条、稳定键；M-03 验收契约。
- **裁决：** 采纳（类别：关键事实错误）。
- **证据：** `crates/automed/src/gate_queue.rs:183` 的 `gate.queued` 载荷只有 `node/job/checkout/position`；`scheduler.rs:3874–3884` 的 `gate.started` 才带 `kind/sha`；
  `task.rs:1023–1032` 允许门禁同因失败后从原节点重跑，`scheduler.rs:2503–2506` 只 `DiscardGateRuns`、不改提交。另核对 `gate_queue.rs:164/183` 与 `setup.rs:161`：
  `admit` 的 `job` 为 `"gate"` 或 `"setup"`，准备命令的排队也发 `gate.queued`，原设计未区分。
- **修改落点：** 「背景与现状」事件一节新增门禁事件字段说明；「执行时间线」门禁条目改为「一次执行」合成规则（每个 node 至多一条未结束执行，
  `queued` 绑定后续 `started`，`result` 绑定运行中的执行，同提交再次开始必为新条目，重复开始标「已中断」，`job: setup` 不进门禁条）；稳定键改为
  `gate:<node>:<seq>`；M-03 预期与负向对照补对应用例。
- **复提计数：** 0

### D1-P02 `kind: repro` + `ok: false` 被当成「复现失败符合预期」

- **主张摘要：** `legacy_kind` 会给没有快检的 Bug `fix_gate` 等也标 `repro`；`ok` 只是退出码为 0；真正是否复现由 `bug.reproduced` / `bug.repro_missed` 决定（超时、126/127 为未复现）。
- **设计位置：** 「执行时间线」门禁名称与状态；「阶段条」Bug 复现检查文案；M-02、M-03 验收契约。
- **裁决：** 采纳（类别：违反任务目标）。
- **证据：** `scheduler.rs:3686–3694` 的 `legacy_kind`；`scheduler.rs:3917–3928` 的 `ok = exit_code == 0`；`scheduler.rs:2026–2066` 的 `reproduced` 判定与 `why`（`timed_out` / `not_runnable` / `exited_zero`）。
- **修改落点：** 门禁名称改按**节点**决定（`repro_check` → 复现检查，`fix_gate` → 修复后回归）；「符合预期」只由 `repro_check` 之后的 `bug.reproduced` 给出，
  `bug.repro_missed` 按 `why` 分三种，业务事件未到时只显示退出码；`fix_gate` 非零一律「未通过」；阶段条同一解释，并在「背景与现状」写明 `kind` 的含义；
  M-02 补四种复现结果，M-03 补对应用例与负向对照。
- **复提计数：** 0

### D1-P03 取消重新排队的任务会把阶段条退回旧阶段

- **主张摘要：** 取消时只取最后一条 `active` 节点；带 `resume` 的 `queued` 状态由内核直接写入、没有 active 事件，批准后排队再取消会退回批准设计。
- **设计位置：** 「阶段条」`cancelled` 一条；M-02 验收契约。
- **裁决：** 采纳（类别：违反任务目标，R-02「已取消停在出事的阶段」）。
- **证据：** `scheduler.rs:2118–2132` 槽位满时 `transition.next = Requeued { resume }`，`2205–2209` 直接写入 `task.updated`；`task.rs:905–913` 允许任意非终结状态取消；
  `task.rs:362–372` `TaskState::node()` 对 Active/Paused/Requeued/Stopped/Failed 都返回节点。
- **修改落点：** 取消改为：从取消那条 `task.updated` 之前倒找第一条带节点的状态（`active.node`、`paused/queued.resume`、`stopped/failed.at`），沿用正常映射；
  草稿与首次排队无节点时保留降级；M-02 第 5 条与负向对照补「批准 → 重新排队 → 取消」。
- **复提计数：** 0

### D1-P04 进程清理依赖最后一个用例

- **主张摘要：** 清理放在「最后一个用例」，`afterAll` 只 `core.stop()` 后删目录；等待超时、断言失败或 `task.stop` 失败时子进程会在目录被删后存活。
- **设计位置：** 「收尾不留进程」；M-01 验收契约。
- **裁决：** 采纳（类别：内部逻辑矛盾，违反任务硬性约束 8 与 A-12）。
- **证据：** 设计自述「停内核不会带走它们」；`apps/web/server/core/supervisor.ts` 的 `stop()` 只停 sidecar，`sidecar.ts` 通过关 stdin 与杀内核子进程停止，不回收假 CLI。
  兜底路径确实在不检查进程的情况下 `rmSync`。
- **修改落点：** 重写「收尾不留进程」：`process-leak.ts` 导出幂等 `teardown`（各步独立 try/catch：停服务 → 尽力停任务 → 停内核 → 按 `leftoverProcesses` 的 pid TERM/KILL 有界等待 →
  确认无残留才删目录，否则保留目录并报错）；`afterAll` 与 `beforeAll` 失败路径都无条件调用；「最后一个用例」仅保留为正常出口的断言；M-01 的收尾用例与负向对照补 `teardown` 的行为；
  夹具一节同步。
- **复提计数：** 0

### D1-S01 操作矩阵证明范围（非阻塞建议）

- **主张摘要：** 矩阵每状态只实际调一个可用操作，不能证明每个「状态×操作」组合。
- **裁决：** 部分采纳——不属于六类，不触发轮次；设计里如实写明证明范围，并记入 Backlog B-07。
- **修改落点：** 「操作表与内核的交叉核对」第 3 层补充说明；`## Backlog` 新增 B-07。
- **复提计数：** 0

## 定稿判断

四项阻塞问题均已按源码证据改写，同类位置（所有排队/开始/结果、需求与 Bug 的 `gate / fix_gate / merge_gate / main_check`、所有 `kind: repro` 节点、
所有带 `resume` 的状态、全部真实内核测试文件的收尾）已一并处理。改写只收紧了展示与测试的规则，没有改变取数通路、里程碑划分与验收命令集，
六个里程碑仍各有验收命令、预期、负向对照。本轮自检没有剩余的六类问题，故状态块 `status` 改为 `实现中`，`current-milestone` 取第一个开放的 M-01，
`design-round` 加到 1/15，里程碑表保持完整的六行。

提交由 Autome 兜底的情形：若本轮提交被拒，改动留在工作区，由 Autome 会话结束后统一提交。
