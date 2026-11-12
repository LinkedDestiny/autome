# 实现 #13：修复全量门禁中的端到端时序不稳定

## 范围与依据

起始 HEAD 为 b4bedce（上一实现提交 b65e21b，其后仅生成简报）。按本轮用户指令只修复门禁，不推进新里程碑；M-01 至 M-04 已由审计关闭，状态原样保留。implementation-round 更新为 13/20。上一审计 #10 没有退回缺陷；本轮发现的问题对应任务 §5 约束 1、§6 验收 1 和 M-04 的 `sh scripts/ci-local.sh`。

宿主提供的汇总没有失败用例名称，因此不能断言本轮抓到的失败就是 b65e21b 上的唯一原因。当前代码原样运行完整门禁，两遍 end_to_end 均 96/96 通过；随后对设计 Backlog B-A2-01 所指用例原样重复，实际捕获两种失败。

## 红灯与修复

原有项目用例：`bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it`，无需把临时检查搬入测试体系，它本来就在 `crates/automed/tests/end_to_end.rs` 中。没有新增、删除或忽略用例。

1. 原样命令 `cargo test --test end_to_end bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it -- --exact --nocapture` 重复运行，第 5 次失败：`legacy loop did not settle`，本次耗时 16.06 秒，命中该用例独有的 15 秒期限。前 4 次通过。日志：`.autome/output/r13-legacy-red.txt`。
2. 同一命令加现有诊断开关 `AUTOMED_KEEP_SANDBOX=1`，首轮失败：断言 `left: Some("retro")`、`right: Some("implement")`，耗时 20.05 秒。日志：`.autome/output/r13-legacy-retained.txt`。只保留测试临时目录，不改变产品行为。
3. 因果证据：`dispatch::task_trigger` 在 `apply_trigger` 启动实现会话后立即执行 `scheduler::tick`，快速替身会话可以在该 tick 中被收取并继续推进。因此 RPC 返回后直接断言短暂的 implement 状态存在竞态，不是旧 Bug Loop 路由错误。
4. 修复：第一个替身会话在写文档前等待文件屏障；观察到 implement 节点和 Impl 会话后才写 release 文件。后续等待复用已有 `resume_wait`（90 秒上限、每次 tick 仍检查 errors），替代独有的 15 秒循环。并未增加固定睡眠来猜测时序。保留 await_merge、会话数 3、merge 后 Done、无 bug.loop_started 的断言。产品代码和门禁脚本不变。
5. 修复后同一精确命令连续 **30/30** 通过，退出码 0；每次仍为 1 个通过、95 个过滤，共执行同一回归用例 30 次，不把重复次数当新增用例数。日志：`.autome/output/r13-legacy-green.txt`。

同类检查范围：检查该用例从审批、Impl 会话观察到全流程完成的全部等待点，以及现有 World::settle、resume_wait 和其它有界等待。全流程的 15 秒限制仅此一处；其它短等待针对单个 pid/exit 标记，不为本轮复现问题扩大改动。移除设计中已解决的 B-A2-01 条目。

## 命令与结果

- 修复前 `sh scripts/ci-local.sh`：退出码 1，Rust 全量 1124 通过、独立 End to end 96 通过；Desktop 104（103 通过、1 跳过）；Web 225（198 通过、11 失败、16 跳过）。唯一失败步骤为 Web test，日志 `.autome/output/r13-gate-red.txt`。文件名 red 表示复现轮日志，不表示该轮 End to end 失败。
- Web 错误明确为 `listen EPERM: operation not permitted 127.0.0.1`，server 子进程另伴随 `EMFILE` watcher 输出。当前会话禁止提权，不能在本会话证明宿主网络监听测试全绿；不改测试、不跳过门禁来消除环境错误。
- `cargo fmt --all -- --check`：退出码 0，日志 `.autome/output/r13-fmt.txt`。`git diff --check` 通过；`git diff --exit-code 2553923 -- .autome/config.toml` 通过，配置未改。

- 修复后再次原样运行 `sh scripts/ci-local.sh`：**退出码 1，用时 663 秒**，日志 `.autome/output/r13-gate-after.txt`。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均通过；唯一失败步骤仍为 Web test，错误仍含 `listen EPERM: operation not permitted 127.0.0.1`。不能宣称整条门禁全绿。
- 最终 Rust 全量 **1124** 个通过（CLI 22+11、domain 355、automed lib 617、e2e 96、prompt_rendering 16、real_cli 7），相对上一审计 #10 的 1124 个 **差 0**。独立 End to end **96/96** 通过，相对修复前完整运行的 96 个 **差 0**。重复运行的 e2e 与 Adapter flags 1 个不重复计入唯一用例总数。
- Desktop **104** 个（103 通过、1 跳过），Web **225** 个（198 通过、11 失败、16 跳过）；相对本轮修复前同命令基线分别 **差 0、差 0**，Web 总数也与用户给出的宿主 225 个一致。三个体系唯一用例共 **1453**，相对本轮修复前 1453 个 **差 0**。没有新增、删除或隐藏测试层；Web 与宿主成功数的差来自本机监听权限，Desktop 的 1 个既有跳过没有变化。
- 状态块必需字段各出现一次，implementation-round 严格为 13/20，里程碑表与起始 HEAD 完全一致；`git diff --check` 通过。

## 标待审之前的自审清单

本轮不新增待审里程碑，仍逐项检查修复范围：

1. **情形表逐行**：旧 Bug 任务停在 AwaitDesignApproval → 审批启动 Impl → 实现文档待审 → 审计文档已完成 → 复盘 → AwaitMerge → 合并 Done；现有用例的节点、角色、会话数及无新 Bug Loop 事件断言全部保留，精确重复 30 次通过。未配 setup 的路径仍由全量套件检查。
2. **失败分支**：快会话在审批返回前结束与正常流程超过独有 15 秒期限，两类原样失败均已记录。调度错误仍被 resume_wait 的 errors 断言拒绝，90 秒仍为硬上限。拒绝授权、读盘失败、准备超时、外部服务不可达不适用本轮测试同步修复，未修改准备执行层。Web 服务不可达由本机 EPERM 实际验证为环境阻塞，没有冒充通过。
3. **全部迁移**：屏障未释放时保留 Impl 会话与 implement 节点；释放后运行原有实现→审计→复盘→待合并→Done 迁移，最终会话数仍是 3，未启动新 Bug Loop。没有新增产品状态或迁移。
4. **域边界**：覆盖快速完成的会话与耗时波动的全流程，观察状态由显式屏障保证，完成由有界条件轮询保证。空字符串、闰日、最大数值等不适用，本轮没有新增输入域或解析逻辑。

## 提交与后续

本轮结束前提交测试修复、证据、状态块与一行 retro；不启动下一会话。最终门禁以 Autome 在本轮提交上运行的结果为准。H-01/H-02 仍保留人工验收范围。
