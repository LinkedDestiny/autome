# 两个工作区 Bug 端到端用例等待误报定位

status: 已定位
fix-round: 3/3
复现命令: cd crates/automed && cargo test --offline -p automed --test end_to_end waiting_regression -- --nocapture
根因: 测试基础设施把未到期重试排除在未完成工作之外，并在多处条件等待中独立使用短截止时间，导致提前返回或正常推进时超时报错。
复核: 待复核
repos: autome-v2

## 现象

任务卡描述 main `281a575` 的两个工作区用例在 30 秒处失败，main `fb13dbd` 的 legacy 用例在 15 秒处失败；另并入 T-34 的限流重试提前返回问题。本轮以任务卡全部补充后的范围为准。

当前定位基线为 `664647a`。当前项目是单 Git 仓库 autome-v2，含 Cargo workspace；实际改动成员为 `crates/automed`。测试中的 a、b 是临时构造的成员仓库，不是本任务需修改的仓库。复现命令从本 worktree 根目录进入 Cargo 成员目录执行。

任务卡所列历史 MainCheck 与重跑日志在本 worktree 中不存在，历史次数和耗时仅作为用户提供的证据，本轮未声称重新验证历史满载现场。

## 复现

新增两个测试，均位于 `crates/automed/tests/end_to_end.rs`，以 `waiting_regression` 过滤运行：

- `waiting_regression_settle_keeps_future_retry_pending`：参考对象库 `3b07a52` 的定位测试，创建真实任务及 Store 状态。依次验证无重试为静止、已到期重试为未完成，再设置 9999 年的未到期重试，执行真实 scheduler tick，确认无错误、无运行会话、任务仍在 adjudicate 且重试未消失。最后验证清除重试后静止，并断言清除前应为未完成。当前实现最后一个断言失败。
- `waiting_regression_tick_waits_do_not_own_deadlines`：源码契约回归，检查三个问题用例、旧快照等待用例和 resume_wait 不再各自声明 deadline，以锁定任务卡要求的共用等待入口。当前实现报告这些入口仍自持截止时间。它不是机器负载模拟或性能测试，不足以独立证明满载时的实际耗时；合并后仍需由修复轮验证共用辅助函数的 90 秒预算及所有调用点。

测试不依赖网络、负载制造或临界时间竞争；未来时间固定到 9999 年。需要本地 Cargo 依赖缓存和 Git，缺少 Git 将显式失败而非跳过。

本轮三次运行均得到两个断言失败，没有命令超时或编译错误。原始日志在 `.autome/output/locate-regression-1.log` 和 `.autome/output/locate-regression-2.log`；第一次耗时 0.26 秒。第二次补充了旧快照等待入口覆盖，耗时 0.40 秒；第三次按状态块的完整命令从成员目录执行，耗时 0.25 秒，退出码 101，日志为 `.autome/output/locate-regression-final.log`。失败文本分别为：

- `settle must not return while an unfinished task has a future pending retry`
- `scheduling waits must share the 90s hang detector instead of owning deadlines`

## 根因分析

1. `World::settle()` 每次 tick 后立即用 `!has_pending_work()` 判定退出。`has_pending_work()` 对 pending_retry 额外要求 `at <= now_iso()`。因此没有运行会话、节点为 adjudicate、重试尚未到期时，会把“暂时不能调度”当成“工作已结束”。回归使用真实 Store 和 scheduler 验证此谓词，未等待未来重试真正到期，也未让 settle 陷入 90 秒等待。
2. 三个问题用例等待的都是业务状态（任务静止、等待用户或失败），并无耗时性能断言。它们使用 15/30/30 秒局部 deadline；`World::settle()` 已说明 90 秒是挂死探测预算。机器竞争会消耗墙钟时间，局部截止不能区分正常慢推进与挂死。
3. 全文件等待点核对（行号为定位基线）：World::settle 358、WorkspaceWorld::settle 2146、resume_wait 2821、旧快照等待 4939 已使用 90 秒，但仍各有循环，修复轮应纳入统一入口。4132、4521、5115 为上述短截止调度循环。468 的 exit-marker 等待以及 2796/2808 的 PID 启动/退出等待不调用 tick，不属于手写 tick 循环，也没有明确的性能预算断言；尤其 exit-marker 等待有意不回收会话，不宜机械替换为会推进调度的函数。是否统一其非调度等待实现应保持原语义。
4. 本轮证据支持测试等待逻辑问题，没有证据证明产品 scheduler 卡死。现有业务状态、会话数、文件内容和错误断言应全部保留。修复 future-retry 判定后，原来用 settle 等待“已安排但未到期重试”的脚本需改为明确等待重试状态，避免等待未来很久的重试。

## 修复说明

修复第 3/3 轮：执行 `git rebase main`，将本分支 rebase 到本地默认分支 `9b5750b07fe9692111ea86fcb1fa8a30f73a84fb`，解决 `crates/automed/tests/end_to_end.rs` 两处冲突后执行 `git add` 与 `git rebase --continue`，rebase 已完成，未使用 merge。冲突来自 main 将两个工作区用例的局部等待改为 90 秒；解决时保留本任务共用 `scheduling_wait` 的 90 秒预算、调度错误检查及原业务状态判断。main 的新增测试全部保留。rebase 前后文件差异仅为 main 新增的三个连续门禁失败处理用例及辅助函数；两项 waiting_regression 定位测试逐字一致，未修改、删除、跳过或弱化。祖先关系检查确认上述 main 提交已包含在本分支。

本轮原复现命令退出码 0：2 passed、0 failed（2.50 秒），日志 `.autome/output/fix3-regression.log`。

本轮配置门禁 `sh scripts/ci-local.sh` 已完整执行，退出码 1，用时 660 秒，日志 `.autome/output/fix3-gate.log`。Format、Clippy、Test、End to end、Adapter flags、Contract、Web lint、Web typecheck、Web build 通过；两遍端到端均为 103 passed、0 failed（122.46 秒、175.63 秒）。失败项如下：

- Desktop：104 passed、1 failed、0 skipped。`test/dom.test.js:71` 的 Electron DOM harness 报 `dom-harness.js exited null`，stderr 为空，断言 `null !== 0`；退出原因未确认，不将其宣称为已证明的环境故障。
- Web test：4 个文件失败，11 failed、271 passed、19 skipped。日志报告 `listen EPERM: operation not permitted 127.0.0.1` 和 `EMFILE: too many open files, watch`，本机监听被拒且文件监听资源不足。

未修改、删除、跳过或弱化任何测试，未修改 Desktop/Web 无关代码。完整门禁未通过，需宿主复跑并确认 Electron harness 退出原因及 Web 环境限制。未额外运行 package 或历史满载压力测试；本轮验证绑定上述 rebase 目标，不代表未来 main 的合并后验证。

以下为第 2/3 轮历史记录：

修复第 2/3 轮：将本分支 rebase 到本地默认分支 main（`a0edb2569552abc0eed3ac18bdbcfc6596a3fde6`），逐个解决 `end_to_end.rs` 冲突并完成 rebase，未使用 merge。保留 main 新增的 setup 测试与 legacy 实现会话释放信号，同时保留本任务的共用调度等待、重试判定和定位回归测试。两项 waiting_regression 测试与 rebase 前逐字比对一致；main 已通过祖先关系检查。

本轮原复现命令退出码 0：2 passed、0 failed（6.72 秒），日志 `.autome/output/fix2-regression.log`。本轮运行配置门禁 `sh scripts/ci-local.sh`，退出码 1，用时 1451 秒，日志 `.autome/output/fix2-gate.log`。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均通过；两遍端到端均为 100 passed、0 failed（225.05 秒、245.69 秒）。唯一失败项为 Web test：4 个文件失败，11 failed、271 passed、19 skipped；日志出现 `listen EPERM: operation not permitted 127.0.0.1` 和 `EMFILE: too many open files, watch`。未修改、删除、跳过或弱化测试，完整门禁未通过，仍需宿主在允许本机监听且文件监听资源充足的环境重跑。未额外运行 package 或历史负载压力测试。

提交前再次检查发现本地 main 已推进到 `9b5750b`。本轮 rebase 与门禁绑定开始时的 `a0edb2569552abc0eed3ac18bdbcfc6596a3fde6`，该提交仍是本分支祖先；不能作为新 main 的合并后验证证明，后续由 Autome 继续调度 rebase 和合并前门禁。

以下为第 1/3 轮历史记录：

修复第 1/3 轮，仅调整 `crates/automed/tests/end_to_end.rs` 的测试基础设施及相关等待调用，未修改产品 scheduler。

- 提取共用 `scheduling_wait`，统一调度条件等待的 90 秒挂死探测预算与 20 毫秒轮询间隔。World、WorkspaceWorld、resume_wait、legacy 用例、两个工作区用例及旧快照等待均使用该入口，保留原业务状态、错误、会话数和文件内容断言。
- 单仓库及工作区共用 `has_pending_work`，未完成任务只要存在 pending_retry，即使尚未到期也属于未完成工作。
- 限流用例改为明确等待“已安排重试且无运行会话”，保留重试日期、角色、节点、事件和会话历史断言，避免把安排到 2100 年的重试当作静止。
- 首次全量门禁发现另 6 个旧用例的脚本故意不覆盖完整流程，原先也依赖未到期重试被误判为静止；清洁工作区、repos 保留、两类取消、停止恢复及拒绝设计反馈用例改用共用 `wait_for_retry` 等待，并保留原有业务断言。补齐后单独运行完整端到端测试：83 passed、0 failed，用时 77.63 秒，日志 `.autome/output/fix-e2e.log`。
- exit-marker 与 PID 启动/退出等待不推进调度，保持原语义。定位轮新增的两个 `waiting_regression` 测试逐字比对未变，未删除、跳过或弱化。

验证：

- 修复前执行状态块原复现命令：两项测试均按预期断言失败，日志 `.autome/output/fix-before.log`。
- 修复后执行同一命令：2 passed、0 failed，日志 `.autome/output/fix-after.log`。
- 全文件等待点已核对：调度循环只由共用入口持有 90 秒 deadline，余下三个局部 deadline 均为非调度等待。
- 首次项目门禁日志为 `.autome/output/fix-gate.log`，其端到端测试暴露上述 6 个旧调用点；补齐后重新执行 `sh scripts/ci-local.sh`，退出码 1、耗时 323 秒，日志 `.autome/output/fix-gate-final.log`。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均通过；其中两次端到端均为 83 passed、0 failed。唯一失败项为 Web test：4 个文件失败，11 failed、271 passed、19 skipped；HTTP 监听被当前沙箱以 `listen EPERM: operation not permitted 127.0.0.1` 拒绝，另有 `EMFILE: too many open files, watch` 导致服务测试启动失败。未修改或跳过这些测试，完整门禁未通过，需宿主在允许本机监听且监听器资源充足的环境重跑。
- 未做历史满载现场复现或额外负载压力测试；90 秒是挂死探测上限，不代表任意负载下都不会超时。

## 复核意见

复核第 1/3 轮：通过。
- 复现命令独立重跑：2 passed（定位提交时按预期失败）；完整 end_to_end：83 passed、0 failed（112 秒）。
- 回归测试：`git diff 1f41ab4 cce9b86` 中两个 waiting_regression 测试零改动，未删除、跳过或忽略；被移除的断言仅为各手写循环的 deadline 断言，业务状态、错误、会话数断言保留，改由共用 `scheduling_wait` 持有。
- 根因站得住：`has_pending_work` 把未到期 pending_retry 当静止，导致 settle 提前返回；修复让未完成任务的重试始终算未完成，并统一 90 秒挂死预算。
- 范围：仅改测试基础设施 end_to_end.rs，产品 scheduler 未动；6 个依赖旧误判的用例改用 wait_for_retry 属必要联动。
- 备注：修复轮报告 Web test 因沙箱 listen EPERM 失败，属环境限制，与本改动无关（未改 Web）；合并前门禁需在宿主复跑。

复核第 2/3 轮：通过。
- 复现命令独立重跑：2 passed、0 failed（0.57 秒）；完整 end_to_end 独立重跑：100 passed、0 failed（41.31 秒）。
- 回归测试：`waiting_regression_settle_keeps_future_retry_pending` 自定位提交 185e8cd 到当前 HEAD 逐字一致（函数体哈希相同），两个 waiting_regression 测试均存在，未删除、跳过或弱化。
- 根因站得住：未到期 pending_retry 被排除在未完成工作外，加上各处局部短截止；rebase 后仍由共用 `scheduling_wait` 与 `has_pending_work` 承担，与第 1 轮结论一致。
- 范围：`git diff 185e8cd HEAD -- crates/automed/src` 为空，产品代码未动；本轮只是 rebase 冲突解决，保留 main 新增的 setup 测试。
- 备注：Web test 在沙箱因 listen EPERM / EMFILE 失败，属环境限制，本改动未触及 Web；修复轮已如实说明 main 在 rebase 期间继续推进，合并前门禁须在宿主基于最新 main 重跑。
