# 驳回设计未携带已做的 Backlog 表态

status: 已定位
fix-round: 4/4
复现命令: cargo test --offline -p automed --test end_to_end rejecting_design_carries_pending_decisions_and_approval_starts_implementation -- --exact --nocapture
根因: Reject 原先未携带和租用表态，第 1 轮已修复；另有 task_trigger 在启动会话后立即 tick 的时序竞争，快速完成的实现会话会在批准命令返回前被收割并推进到 audit。
复核: 待复核
repos: .

## 现象

设计批准卡点上，B-01/B-03 表态 ignore、B-02 表态 include 后驳回设计。驳回意见到达新设计会话，但三条表态未进入 prompt。设计、评审、裁决正常完成并再次停在批准卡点后，表态仍未消费；批准启动的节点为 `design`，不是预期的 `implement`。

本地自动化复现与任务卡的核心现象一致；未访问 T-61 的生产数据库或历史会话，未独立核实其中约 25 分钟的耗时。

## 复现

- 基线：`116a87c`。本项目是单 Git 仓库，根目录包含 Cargo workspace；`repos: .` 指当前仓库，不存在需要切换进入的 Autome 多仓库成员目录。复现命令在当前 worktree 根目录执行。
- 测试：`crates/automed/tests/end_to_end.rs` 中的 `rejecting_design_carries_pending_decisions_and_approval_starts_implementation`。
- 使用现有 `World` 端到端夹具，通过真实命令分发执行 `task.decide`、`task.reject`、`task.approve`；运行真实 Git、调度器、SQLite 存储及会话包装脚本，仅以本地脚本替代模型，不访问外部服务。等待使用夹具的有界状态轮询，不依赖固定睡眠确定业务结果。
- 前四个会话到达批准卡点；提交一条 include、两条 ignore；第 5 个设计会话收到不重复 Backlog 正文的驳回意见，第 6/7 个会话完成评审和裁决，再次批准。
- 断言同时检查第 5 个 prompt 中三条表态的编号、正文和纳入/忽略方向，表态正常结束后已消费且租约已释放，以及再次批准立即进入实现。检查前还验证表态记录数、驳回意见送达、正常返回批准卡点及会话数，避免夹具失败冒充复现。收集批准节点后暂停新会话，避免清理夹具时仍有进程访问文件。

失败断言报告：

```text
missing decision text in rejection prompt: ["B-01", "B-02", "B-03"]; unconsumed decisions after successful redesign: ["B-01", "B-02", "B-03"]; node after approval: Some("design") (expected implement)
```

运行日志存于 `.autome/output/reject-decisions-first.log` 和 `.autome/output/reject-decisions-repeat.log`，不提交临时输出。无表态驳回对照使用已有用例 `rejecting_a_design_re_runs_it_carrying_the_feedback_verbatim`，日志为 `.autome/output/reject-no-decisions-control.log`。

实跑结果：回归测试两次均以退出码 101 在上述业务断言失败（不是超时或编译失败）；第二次包含完整表态方向检查及暂停清理。无表态驳回对照 1 passed、0 failed；`git diff --check` 通过。本轮未运行全量测试或产品门禁，未执行任何产品修复。

## 根因分析

1. `crates/autome-domain/src/task.rs:1223` 附近的 `Trigger::Reject` 转换仅生成 `Inject::DesignFeedback`，没有表达驳回意见与表态需要同时送达。任务卡中的旧称 `Inject::Rejected` 已变为 `DesignFeedback`。
2. `crates/automed/src/scheduler.rs:2481` 仅当注入为 `Inject::Decisions` 才调用 `carry_decisions`。驳回会话得到空表态列表，没有设置 `carried_by`。
3. `crates/automed/src/launcher.rs:464` 的驳回分支只渲染反馈，`:499` 的表态分支才调用 `render_decisions`。测试确认反馈可达而表态全部缺失，排除了驳回命令本身未生效。
4. 正常会话结束调用 `settle_decisions`（`scheduler.rs:370,404`），但 `crates/automed/src/store.rs:1748` 起的消费操作只更新绑定当前会话的记录。未取得租约的三条表态不会消费，属于未送达的后果，而非已送达后的消费失败。
5. `task.rs:1202` 的批准分支检查 `ctx.decisions.any()`，发现仍有未消费表态后启动 `Inject::Decisions` 设计轮。因此仅把驳回意见里的业务文字写入方案不会改变账本状态，仍触发多余设计。当前判断已不限于任务卡所述的 `decisions.included > 0`。
6. 第 2 轮确认另一个时序问题：表态消费后，领域批准转换正确进入 `Implement`；但 `dispatch.rs` 的 `task_trigger` 在 `apply_trigger` 启动实现会话后无条件调用 `scheduler::tick`。若实现会话此时已完成，`tick_locked` 会收割会话并推进到 `Audit`，因此命令返回时的节点受进程速度影响。宿主失败与本地修复前重复运行均出现表态检查通过、节点为 `audit` 的结果。

任务卡引用的 `docs/plans/2026-10-11-decisions-take-effect-next-round.md` 在当前 checkout 不存在，未将其内容作为已核实证据。现有表态租约测试与当前生产实现足以支持以上因果链。

## 修复说明

第 4/4 轮验证与收尾：

- 本轮开始时 HEAD 已包含 `28d28fd`，仅将回归测试中的单行 `if/else` 按 rustfmt 展开，上一轮的格式阻塞已解除。本轮未修改产品代码或定位轮回归测试，仅更新本记录。
- `cargo fmt --all -- --check` 通过（退出码 0）；原复现命令通过（1 passed、0 failed）；直接相关子集 `cargo test --offline -p automed --lib rejecting` 通过（2 passed、0 failed）。日志分别为 `.autome/output/fix-round-4-fmt.log`、`.autome/output/fix-round-4-repro.log`、`.autome/output/fix-round-4-rejecting.log`。
- 保持 `status: 已定位`、原复现命令、根因和 repos 不变，按本轮预算更新 `fix-round: 4/4`，重置 `复核: 待复核`。`git diff --check` 通过。未运行全量门禁 `sh scripts/ci-local.sh`，由宿主合并前执行；未启动下一会话。

第 3/3 轮检查与阻塞：

- 宿主报告仅 Format 失败。本轮单独执行 `cargo fmt --all -- --check`，退出码 1，唯一差异为 `crates/automed/tests/end_to_end.rs:2631` 的单行 `if/else` 需展开为多行；日志为 `.autome/output/fix-round-3-fmt-before.log`。无需改动产品代码。
- 用户明确要求不要修改定位轮回归测试，因此本轮尚未格式化该测试，也未修改格式门禁或添加跳过配置。已请求允许仅修正该处格式，等待确认；Format 尚未通过，本轮不能报告修复完成。
- 原复现命令通过（1 passed、0 failed），直接相关子集 `cargo test --offline -p automed --lib rejecting` 通过（2 passed、0 failed）；日志分别为 `.autome/output/fix-round-3-repro.log`、`.autome/output/fix-round-3-rejecting.log`。
- 状态保持已定位，原复现命令及 repos 保持不变，轮次更新为 3/3，复核重置为待复核。未运行全量门禁 `sh scripts/ci-local.sh`，未启动下一会话。

第 2/3 轮修复：

- 仅调整 `dispatch.rs::task_trigger`：比较触发前后的运行会话 ID；本次命令启动新会话时，不立即追加调度收割，由下一次调度推进。没有启动新会话时保留原调度行为，继续处理释放槽位、暂停会话和待执行的核心步骤。
- 保留第 1 轮表态送达及消费修复；定位轮回归测试未修改。
- 修复前原命令先通过两次，第三次退出码 101，报告表态缺失和未消费列表均为空、批准节点为 `audit`，与宿主一致。日志为 `.autome/output/fix-round-2-before.log`、`fix-round-2-before-repeat-{1,2}.log`。
- 修复后原复现命令通过，并额外连续重复三次全部通过（每次 1 passed），日志为 `.autome/output/fix-round-2-repro.log` 和 `fix-round-2-after-repeat-{1,2,3}.log`。
- 直接相关子集全部通过：`cargo test --offline -p automed --lib approving`（3 passed）、`--lib rejecting`（2 passed）、`--lib decision`（7 passed）；端到端子集 `rejecting_a_design_re_runs_it_carrying_the_feedback_verbatim -- --exact`（1 passed）、`paused_`（6 passed）、`the_parallel_limit_queues_the_fourth_task_and_releases_it_on_completion -- --exact`（1 passed），均使用 `cargo test --offline -p automed --test end_to_end`。日志为 `.autome/output/fix-round-2-{approving,rejecting,decisions,control,paused,slots}.log`。
- `git diff --check`、`rustfmt --edition 2024 --check crates/automed/src/dispatch.rs` 通过；与定位提交 `d4a3f0f` 对比，回归测试文件无差异。未运行全量门禁 `sh scripts/ci-local.sh`，由宿主合并前执行。

第 1/3 轮修复：

- `scheduler.rs` 将 `Inject::DesignFeedback` 纳入携带表态的会话判断，并在启动驳回设计会话前调用现有 `carry_decisions`。继续复用启动失败释放、会话正常结束消费、异常结束释放的租约机制，不提前消费未送达的表态。
- `launcher.rs` 在原文驳回意见之后调用现有 `render_decisions`，同时送达表态编号、正文和方向；没有表态时不追加内容。未修改领域转换或注入序列化结构。
- 定位轮回归测试保持原样。修复前执行原复现命令，退出码 101，仍报告三条表态缺失、未消费及批准后进入 design；修复后相同命令通过（1 passed）。日志为 `.autome/output/fix-round-1-before.log` 和 `.autome/output/fix-round-1-repro.log`。
- 直接相关子集全部通过：`cargo test --offline -p automed --lib decision`（7 passed）；`cargo test --offline -p automed --lib rejecting`（2 passed）；`cargo test --offline -p automed --lib design_feedback_is_injected_verbatim`（1 passed）；`cargo test --offline -p automed --test end_to_end rejecting_a_design_re_runs_it_carrying_the_feedback_verbatim -- --exact`（1 passed）。日志分别为 `.autome/output/fix-round-1-{decisions,rejecting,feedback,control}.log`。
- `git diff --check` 和修改源码的 `rustfmt --edition 2024 --check crates/automed/src/launcher.rs crates/automed/src/scheduler.rs` 通过。`cargo fmt --all -- --check` 未通过，仅报告定位轮回归测试 `end_to_end.rs:2631` 已有格式差异；遵照不得修改回归测试的要求保留原样，详情见 `.autome/output/fix-round-1-fmt.log`。
- 未运行全量门禁 `sh scripts/ci-local.sh`，留由宿主合并前执行；复核: 通过（第 2/3 轮，独立复核）。

- 复现命令连续独立运行 4 次全部通过（1 passed），此前宿主出现的时序失败未再现。
- 额外跑 automed 端到端全量 146 passed、lib 全量 761 passed，均 0 失败。
- 差异检查：自定位提交 d4a3f0f 起，回归测试文件无改动，未删除、跳过或弱化；产品改动仅 dispatch.rs、launcher.rs、scheduler.rs 共 +18/-5。
- 根因成立：task_trigger 启动会话后无条件 tick，快速完成的会话在命令返回前被收割，节点由 implement 变为 audit。修复仅在本次命令新启动会话时跳过 tick，由下一次调度收割；未启动会话时行为不变，范围最小。
- 说明：全量门禁（ci-local.sh）未运行，由宿主合并前执行。

## 复核意见

复核: 通过（第 2/3 轮，独立复核）。

- 复现命令连续独立运行 4 次全部通过（1 passed），此前宿主出现的时序失败未再现。
- 额外跑 automed 端到端全量 146 passed、lib 全量 761 passed，均 0 失败。
- 差异检查：自定位提交 d4a3f0f 起，回归测试文件无改动，未删除、跳过或弱化；产品改动仅 dispatch.rs、launcher.rs、scheduler.rs 共 +18/-5。
- 根因成立：task_trigger 启动会话后无条件 tick，快速完成的会话在命令返回前被收割，节点由 implement 变为 audit。修复仅在本次命令新启动会话时跳过 tick，由下一次调度收割；未启动会话时行为不变，范围最小。
- 说明：全量门禁（ci-local.sh）未运行，由宿主合并前执行。

### 第 3/3 轮独立复核

复核: 打回（第 3/3 轮）。

- 产品修复无问题：原复现命令本轮独立运行 2 次均通过（1 passed）；根因（Reject 未携带表态、task_trigger 启动会话后立即 tick 的时序竞争）成立，回归测试保持有意义，未被删除、跳过或弱化；第 3 轮未改动任何产品代码。
- 阻塞项：`cargo fmt --all -- --check` 本轮复验仍退出 1，唯一差异为 `crates/automed/tests/end_to_end.rs:2631` 的单行 `if/else` 需展开为多行。这是纯格式差异，不改变测试语义，修复轮因“不得修改回归测试”而未处理，格式门禁因此仍会失败。
- 建议：允许仅做 rustfmt 格式化该处（不改断言与逻辑）后，其余无需再改；这需要用户追加轮次或明确放行。本轮按规则不代修，未运行全量门禁。
