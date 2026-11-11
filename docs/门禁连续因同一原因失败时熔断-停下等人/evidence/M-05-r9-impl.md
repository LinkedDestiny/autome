# M-05 r9 实现：宿主门禁超时修复

## 范围与根因

本轮为用户指定的修复轮，不推进新里程碑。起始 HEAD edd19ce（前一实现提交 6c143f5），工作区干净。M-01～M-05 已由审计关闭，本轮保持原状态；implementation-round 为 9/25。

用户提供的 6c143f5 门禁尾部只显示 Test 失败，没有失败用例栈，不能凭尾部断言其根因。上一轮审计现存完整日志 `.autome/output/audit6-ci.log` 则明确记录 `bug_loop_a_workspace_bug_checks_out_every_member_to_locate_then_keeps_only_the_named_repos` 在 end_to_end.rs:4690 的 30s 断言失败，状态为 MergeGate。事件显示 locate、fix、fix_review 均完成，fix_gate 成功，merge_gate 刚启动；三次假会话退出码均为 0。它证明测试期限过紧的真实失败，但不能据此声称已核实 6c143f5 的具体失败栈。

检查同类范围：end_to_end.rs 所有 Instant deadline。World::settle、Workspace::settle 均使用 90s 防挂死期限；仅选仓测试及 `bug_regression_guard_checks_each_workspace_member` 两条完整工作区 Bug Loop 仍使用 30s。将这两处统一为 90s，保留每次 tick 的错误检查、终态要求、所有拒绝及文件保护断言、20ms 轮询间隔。没有更改产品超时、门禁脚本、并行数或跳过条件。设计文档明确列出修改既有测试的两条例外与理由。

## 复现与红绿证据

- 上一轮审计检查已经是项目测试体系里的原用例，无临时检查需要搬入。本轮先运行未改源码的 `sh scripts/ci-local.sh`：Rust Test 通过，e2e 两次均 83/83；未复现原来的 Rust 失败。全命令退出 1，515s；本轮实际红为 Web 测试 `listen EPERM: operation not permitted 127.0.0.1`，与原报告的 Test 红不同。日志 `.autome/output/impl-9-ci-red.txt`。
- 原样定向命令 `cargo test -p automed --test end_to_end bug_loop_a_workspace_bug_checks_out_every_member_to_locate_then_keeps_only_the_named_repos -- --exact`：1/1 通过，48.36s（四个情形合计，30s 原期限按情形计算），日志 `.autome/output/impl-9-focus-red.txt`。同一原命令四份并发均通过，耗时 50.16～51.78s，日志 `.autome/output/impl-9-load-red-{1,2,3,4}.txt`。这些文件名中的 red 表示修复前，不代表它们失败；不伪造本轮红→绿复现。
- 机制性解释：30s 是调度多个真实 Git 操作及子进程的整段等待期限，宿主并发负载影响完成时间。历史失败事件持续推进、停在刚启动的最后门禁，原样重跑通过，支持期限抖动而非固定死锁或脚本失败。修复采用项目既有 90s 防挂死值，仍然有界；不能承诺任何负载下都不会超时。

## 验收

- 修复后原命令 `sh scripts/ci-local.sh`：退出 1，407s，日志 `.autome/output/impl-9-ci-after.txt`。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 全部通过；唯一未通过为 Web test，原因仍是沙箱拒绝 `listen 127.0.0.1`（EPERM）。当前权限模式不允许提权；没有改测试绕过监听或跳过门禁。未取得完整门禁通过证据，最终以 Autome 提交后宿主复验为准。
- Rust Test 总数 **1133**：CLI 23+11、domain 374、automed 库 618、e2e 83、prompt_rendering 16、real_cli 7、T-14 忽略 1；1132 通过、1 忽略，与实现 #7 完整基线及本轮修复前相比均 **差 0**。未新增、删除用例。e2e 在 Test 阶段 83/83（112.98s），独立 End to end 阶段再次 83/83（123.45s），两条被修改的用例均通过。Adapter flags 另选跑 1/1，6 个被过滤，非新增用例。
- Desktop 总数 **105**，104 通过、1 跳过，与实现 #7 及修复前 **差 0**；跳过原因仍为 Electron binary 不存在且下载失败，不能视为真实 Electron 验收通过。
- Web 总数 **225**，198 通过、11 失败、16 跳过；与用户提供的上轮 225 及本轮修复前 **差 0**。上轮宿主环境 225 全过，本轮执行分布变化来自监听权限限制，不是删测。真实 HTTP/WS 部分未完成，不能用总数不变掩盖跳过。
- 项目 `status_block::parse` 通过，5 个里程碑、当前 M-05；日志 `.autome/output/impl-9-status.txt`。`git diff --check` 通过。测试文件差异仅 2 行期限替换及 3 行注释；未改原断言、产品代码或 ci-local.sh。

## 自审清单

1. **情形表逐行**：选仓测试的 `repos: a` 删除未选 b、保留有提交的 b、缺失 repos、未知仓库四种情形，及保护测试修改 a、修改 b、不修改三种情形，全部仍由原测试逐行断言；修复前完整套件均通过，修复后结果见验收。
2. **失败分支**：缺失/非法 repos 必须 Protocol 失败，修改任一受保护 regression.sh 必须在 Fix 被拒；保留原断言。读盘失败、拒绝授权、外部服务不可达不是本次改动涉及的分支，不适用；测试期限仍为有限 90s，超时仍带 w.why 诊断失败。
3. **全部迁移**：两条测试覆盖 locate → repro_check → fix → fix_gate → fix_review → rebase → merge_gate → await_merge，以及非法选仓、篡改回归文件 → Failed；未修改产品状态机。其他产品状态迁移由完整原套件验证。
4. **域边界**：原有空 repos、未知成员、单个选中成员、有/无未选仓库提交，以及 a/b 任一成员修改/均不修改全部保留。数值期限是测试内部常量，不是用户输入域；前导零、闰日、跨年不适用。

H-01 真实桌面人工验收仍未取证。本轮没有启动后续会话；提交后由 Autome 独立重跑门禁决定结果。
