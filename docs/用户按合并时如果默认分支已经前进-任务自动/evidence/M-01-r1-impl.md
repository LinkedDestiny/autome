# M-01 实现轮 #1

结论：验收通过，提交独立审计；本轮仅推进 M-01。无上一轮审计文件，无须搬入审计回归检查。

## 实现

- `task.rs` 新增 `Behind`、`apply_with_merge_request` 与转换输出授权位。旧 `apply` 传入 false；统一在转换返回后计算授权延续，当前状态和下一状态必须同属 rebase/merge_gate 段。抽出 `press_merge` 共用 Backlog 吸收逻辑。
- `git.rs` 新增 `BlockKind`、`MergeCheck`、`MergeAttempt`、`check_merge`、`try_merge`。按脏文件、不在默认分支、落后顺序检查；真正合并失败独立分类；旧 `merge_task_branch` 只映射结果，文案不变。
- 调度器、面板、事件、CLI 尚未接入，按设计留给 M-02 至 M-05；本轮不声称端到端自动合并已可用。

## 验收命令与用例数量

- `cargo test -p autome-domain > .autome/output/m01-domain.txt 2>&1`：退出 0，299 个通过，0 失败、0 忽略；doc tests 0。上一轮无实现验收，以设计记录的 288 为基线，差 +11，来自契约指定 D1–D11。
- `cargo test -p automed --lib > .autome/output/m01-lib.txt 2>&1`：退出 0，548 个通过，0 失败、0 忽略。以设计记录的 545 为基线，差 +3，来自 G1–G3。
- 合计 847 个用例，基线 833，增加 14；没有删除测试。逐个比对 HEAD 的原有测试函数正文：task.rs 77 个、git.rs 31 个原样保留。
- `cargo fmt --all -- --check`、`git diff --check`：退出 0。
- 初次 lib 全量 547 通过、1 失败：G1 曾错误预期在 side 分支检查不存在的默认分支会返回 I/O 错误，实际依照前置条件顺序先返回 NotOnDefault。修正新增测试为回 main 检查不存在的任务分支，再跑上述全量通过。未修改产品规则迁就测试。

## 标待审之前的自审清单

### 1. 情形表逐行

- Merging + Behind，授权输入真假、repos 空/单/多：D1 → Requeued(Rebase)，Action::None，授权真。
- Rebase + Ok，无门禁且有授权：D2 → Merging + RunCoreStep，授权清除。
- Rebase + Ok，有门禁且有授权：D3 → MergeGate + RunCoreStep，授权保留。
- MergeGate + Passed，有授权：D4 → Merging + RunCoreStep，授权清除。
- 上述成功分支无授权：D5 → AwaitMerge；旧有门禁用例仍验证 Rebase → MergeGate → AwaitMerge。
- 脏文件、不在默认分支、落后：G1/G2/G3 分别检查分类与原文；脏与落后同时存在优先脏，用户内容与 HEAD 未改变。
- 实际 git merge 失败：G3 用真实 index.lock 制造失败，检查 MergeFailed、detail 含 index.lock、HEAD 未变且无 MERGE_HEAD。四种拒绝的旧包装返回值与 try_merge 的 detail 逐字一致。

### 2. 失败分支

- 拒绝错误授权来源：D9 覆盖停止后重跑、复盘进入 rebase、段外节点的可接受触发，旧真值不延续。
- 错误结果节点：D11 遍历全部 Node，非 Merging 报 Behind 全拒绝；当前 Rebase/报告 Merging 也拒绝。
- rebase 冲突：D6 → Implement，保留冲突文件注入、清授权；rebase Failed → Failed，清授权。
- merge_gate 失败：D7 → Implement，GateFailed 注入、清授权。
- Git 拒绝四类：G3 全部实际运行；无效引用和空任务分支：G1 返回错误。
- 超时、外部服务不可达：不适用，M-01 是纯状态转换和本地 Git 分类，不新增服务调用或超时处理；门禁运行和调度器失败路径属于后续里程碑。
- 读盘失败：本轮以 G1 的无效 Git 引用错误传播及 D6 的核心步骤 Failed 检查覆盖接口；未模拟操作系统权限拒绝。

### 3. 全部迁移

- 新增/改变的成功迁移：D1–D5 全覆盖，见上述逐行表。
- 段内 Active → Paused → Requeued → Active，以及 Requeued → Active：D8 对 Rebase、MergeGate 两节点分别执行，授权保留。
- Active/Paused → Stopped → Requeued：D8 两节点分别执行，停止及携带过期真值恢复均清授权。
- Active/Requeued/Paused → Cancelled：D8 两节点分别执行，授权清除。
- Rebase → Implement/Failed：D6；MergeGate → Implement：D7；Rebase/MergeGate → Merging：D2/D4；段外 → Rebase：D9。均检查授权清除。
- 自动路径期间新纳入 Backlog：D10 对 Rebase 成功和 MergeGate 通过分别断言回 Implement、消耗决定、增加预算、清授权。
- 其他原有状态迁移由未修改的全量 domain 测试守护；本轮未新增通道。

### 4. 输入域边界

- D1 检查授权真假、repos 为空/单仓/多仓、空 detail；D11 遍历全部节点并检查节点不一致。
- D2/D3 覆盖无门禁/有门禁；D5 覆盖无授权；D10 与 D2/D4 对照 included=1/0。
- G1 检查不存在的任务引用及空串；G2 检查脏和落后同时存在；G3 检查真实锁冲突。
- 日期、前导零、数值最大值不适用：本轮没有新增日期或数值输入域；预算计算沿用原函数。

## 负向对照

逐项临时注入设计指定错误，执行项目测试后恢复源文件；最终全量在恢复后的代码上运行。以下退出码均为 101，且为断言失败而非编译失败：

- 忽略输入授权：`cargo test -p autome-domain --lib requested_`，5 个匹配用例中 3 个失败（D2/D3/D4）。日志 `.autome/output/m01-negative-no-request-routing.txt`。
- Behind 改回 AwaitMerge：`cargo test -p autome-domain --lib merging_behind_the_default_branch`，D1 失败。日志 `.autome/output/m01-negative-behind-blocked.txt`。
- 延续规则不看当前状态：`cargo test -p autome-domain --lib a_rebase_entered_from_anywhere`，D9 失败。日志 `.autome/output/m01-negative-stale-authorization.txt`。
- 落后优先于脏：`cargo test -p automed --lib merge_check_reports_conflicting_dirt`，G2 失败。日志 `.autome/output/m01-negative-dirty-after-behind.txt`。
- git merge 失败误标 Behind：`cargo test -p automed --lib try_merge_returns_the_kind`，G3 失败。日志 `.autome/output/m01-negative-failed-is-behind.txt`。

## 未运行与下一步

本轮未运行 workspace 全量、e2e、clippy、宿主门禁；它们不属于 M-01 验收命令，端到端接入及全量收口按后续里程碑执行。由审计轮独立复验 M-01 后，再推进 M-02。
