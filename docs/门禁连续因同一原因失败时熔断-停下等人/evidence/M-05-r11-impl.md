# M-05 r11 实现证据：宿主门禁修复

- 实现轮 11/25；本轮是门禁修复轮，不推进新里程碑。M-01 至 M-05 原为已完成，全部保持原状态；没有将任何里程碑标成已完成。
- 上轮审计没有退回缺陷或临时检查；本次失败检查原本就在项目测试体系中，保留原用例及其全部业务断言。

## 复现与根因

Autome 在 `6663897d7c6b` 的日志 `/Users/dannie/project/autome-v2/.autome/output/gates/T-28/merge_gate-6663897d7c6b.log` 第 1174–1186 行：`bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it` 期望 `Some("implement")`，实际 `Some("audit")`。原始 Test 阶段 e2e 为 83 通过、1 失败，共 84 个。

首先执行 `sh scripts/ci-local.sh > .autome/output/impl-11-ci-red.log 2>&1`。原代码的 Test 阶段 84 个 e2e 全过，未稳定复现偶发竞态；此命令运行期间完成了下列定向红绿验证，因此这次完整运行不作为最终代码门禁证据。

原样重复命令：`cargo test -p automed --test end_to_end bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it -- --exact`。第 1 次通过，第 2 次退出 101，原断言再次得到 audit 而非 implement；日志 `.autome/output/impl-11-legacy-1.log`、`.autome/output/impl-11-legacy-2.log`。每次选中 1 个，过滤 83 个，没有新增或删除用例。

根因：`dispatch::task_trigger` 先 `apply_trigger` 启动 Impl，再调用 `scheduler::tick`；快速替身进程可以在这两个操作间退出，tick 合法收割并启动 Audit。原测试把审批返回误当成 Impl 仍在运行的保证。不是错误角色路由或多启动了实现轮。

修复：仅在此测试的第一步替身添加文件屏障，保留原节点、Impl 角色断言，断言后创建释放文件，再沿原流程检查 await_merge、3 个会话和最终 done。屏障检测测试根目录存在性，清理后不无限等待。不改生产代码、宿主门禁脚本、超时、成功或拒绝断言。

同类范围：检查 end_to_end.rs 的 implement 节点及 running_session 断言。停止/恢复测试已有文件屏障；另一个 implement 节点断言在冷却等待态且确认无运行会话，不受本竞态影响。

## 验证

- 修复后同一精确命令通过，`.autome/output/impl-11-legacy-green.log`；随后重复 5 次均通过，`.autome/output/impl-11-legacy-green-{1..5}.log`。每次 1 通过 / 0 失败 / 0 忽略，共 1 个，与红灯基线差 0。
- 最终完整门禁：`sh scripts/ci-local.sh > .autome/output/impl-11-ci-green.log 2>&1`，退出 1，用时 650s。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 共 10 步通过；仅 Web test 失败。首次运行也退出 1，用时 756s，仅 Web test 失败。
- Rust Test 共 **1142** 个：1141 通过、0 失败、1 忽略（既有 T-14 日志测试），与 r10 审计的 1141 + 1 基线差 **0**；独立 End to end **84/84** 通过，较本轮原始基线差 **0**；Adapter flags 选中 1 个通过（过滤 6 个）。这些重复执行不另计新增用例。
- Desktop 共 **105** 个：104 通过、0 失败、1 跳过，与首次运行基线差 **0**。跳过源于 Electron 二进制缺失，下载时 fetch failed，是测试原有行为，本轮未改跳过条件。
- Web 共 **301** 个：271 通过、11 失败、19 跳过，与用户给出的 Autome 基线 301 差 **0**。失败集中在 gateway-http、notifier-live、eventbus-live、server 四个文件，均含 `listen EPERM: operation not permitted 127.0.0.1`；后两者另有 watcher `EMFILE`，初始化失败使 19 个用例跳过。没有删测试或修改测试范围。
- 环境负向核对：最小 `node:net` 探针调用 `server.listen(0, "127.0.0.1")` 同样报 EPERM，见 `.autome/output/impl-11-listen-probe.log`；因此当前权限环境无法取得整条命令绿灯，不能把 Rust 修复通过等同于宿主门禁通过。未尝试绕过沙箱、未修改 Web 或 ci-local.sh。
- `git diff --check` 通过。状态块仍为审计已关闭后的 `已完成`，轮次为 11/25，next-action 指向 Autome 提交后复验；里程碑表及 reopen 不变。

## 四项自审

1. **情形表逐行**：本轮不改熔断情形表或读者逻辑；针对旧版 Bug 兼容路径，批准后 Impl 节点、Impl 角色、释放后 await_merge、3 个会话、合并后 done 均由原用例逐项验证通过。全量覆盖结果见上方。
2. **失败分支**：原观察竞态已红灯复现，并由文件屏障修复；拒绝授权、读盘失败、超时、空输入、外部服务不可达不适用本轮测试同步改动，没有新增这些生产分支。
3. **全部迁移**：旧版 Bug 的 AwaitDesignApproval → Implement → Audit → Retro → Rebase → AwaitMerge → Done 继续由真实调度、真实包装脚本及 Git 执行；原最终状态和会话数断言保留。熔断状态机未修改。
4. **域边界**：不适用新增输入域，本轮不新增解析、数值或日期输入；同步边界为释放文件不存在时不能退出、存在时继续，原中间与终态断言分别检查两侧。

## 交付边界

H-01 仍是人工验收，未取证。本轮不启动下一会话、不推送。最终门禁结果以 Autome 在本轮提交上重跑为准。
