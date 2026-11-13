# logtail-live 假会话进程未退出
status: 无法复现
fix-round: 0/3
复现命令: npm --prefix apps/web test -- test/logtail-live-cleanup.test.ts
根因: 静态分析发现停止标记随临时目录立即删除，假 CLI 可永久等待；受沙箱限制尚未完成动态确认
复核: 待复核
repos: apps/web

## 现象

任务卡报告：原测试通过后留下 run_session.sh、fake-cli、tee、render-stream 等孤儿进程，假 CLI 每 50ms 创建一次 sleep，重复测试持续积累进程。

期望：测试结束后两秒内，本次临时目录关联的进程全部退出。本轮不能将环境错误算作 Bug 复现，故状态为「无法复现」，并非判定不是 Bug。

## 复现

工作目录为当前 worktree 根目录；`npm --prefix apps/web` 选择成员目录。基线 HEAD：`65bace83`，macOS，Node `v26.5.0`。

准备命令 `npm ci --prefix apps/web` 与 `cargo build --bin automed` 均成功。最初未安装依赖时的启动失败不计入复现。

新增独立回归测试 `apps/web/test/logtail-live-cleanup.test.ts`：

- 以当前 Node 和本地 Vitest 启动原测试文件，不通过 npx 下载依赖；子测试必须正常退出，环境错误不会被当作进程泄漏。
- 使用专属 TMPDIR/TMP/TEMP 区分本次进程，项目既有 `fileParallelism: false` 保持文件串行。
- 子测试结束两秒后读取进程表，断言本次目录关联进程及其后代为空；finally 捕获并终止残留后代，包括命令行不含临时目录的 render-stream/sleep，再检查清理结果。
- 启动前预检 ps 权限，避免无法检查或清理进程时继续启动子测试。

实际执行：

1. `npm --prefix apps/web test -- test/logtail-live-cleanup.test.ts`：退出 1。首次运行子测试在初始化阶段报 `listen EPERM: operation not permitted 127.0.0.1`，三个测试全部跳过，尚未创建假会话；外层 finally 的 `spawnSync ps EPERM` 又覆盖了原始错误。原始日志保存在 `.autome/output/repro-1.log` 和 `.autome/output/nested-blocked.log`。
2. 修正新增测试的 lint 问题，并将 ps 能力检查移到启动前。`npm --prefix apps/web run lint`、`npm --prefix apps/web run typecheck` 均退出 0。项目没有单独的 Web 格式化脚本，以项目 ESLint 检查新增测试；未使用其他格式化规则。
3. 检查后重跑完全相同的复现命令：退出 1，在启动前报 `spawnSync ps EPERM`，日志 `.autome/output/repro-2.log`。**尚未得到预期的 Leaked logtail-live processes 断言失败，也未证明稳定复现。**
4. quick 所含检查分步执行：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、上述 Web lint/typecheck 均通过；依赖已安装。日志为 `.autome/output/cargo-fmt.log`、`clippy.log`、`lint.log`、`typecheck.log`。没有执行完整门禁。

宿主待补证据：在允许本地监听、ps 查询及本次子进程清理的 macOS 环境，完成以上准备后运行状态块命令至少两次；确认原子测试三项通过，而外层稳定因泄漏进程列表非空失败，并确认 finally 后无本次残留。当前需求、步骤、期望均充分，缺少的是可执行环境与动态证据，不需要用户重新说明需求。不能把超时、缺少命令或权限失败作为成功复现。

## 根因分析

静态证据支持以下机制，动态结论待宿主确认：

- `apps/web/test/logtail-live.test.ts:50`、`:52` 的两个等待循环仅检查 go/stop 是否存在，没有目录消失退出条件或总寿命上限。
- `:83–89` 的 afterAll 写入 go/stop 后删除整个目录，没有等待假会话进程结束。第三项测试已关闭服务并将 running 设为 undefined；其 afterAll 的可选 close 不会等待运行中的假会话。若 CLI 下一次轮询晚于目录删除，就永远无法再看到标记；甚至可能停在 go 循环。
- `crates/automed/src/launcher.rs:1134` 为会话创建独立进程组，注释明确会话允许比驱动进程活得久。`apps/web/server/core/sidecar.ts:266` 起的 stop 关闭 Core 输入并在必要时结束 Core，不等于结束测试创建的独立会话。测试必须承担自己的进程清理责任。
- 原测试的 logtail.stats 与 lsof 验证覆盖订阅和 Web 进程的句柄，未检查操作系统中的独立会话，因此能漏掉该问题。
- 任务卡提到第二会话复位 stop 可能影响第一会话；但正常路径第二项测试先等待 log.end 和 finished，再进入第三项。因此不能直接断定正常路径的第一会话也会泄漏。本轮优先证据是最后一个仍活动会话与 afterAll 删除标记之间的竞争。

已静态核对同目录引用 fake-cli/AUTOMED_CLAUDE_BINARY 的测试及 fixtures：notifier-live、notifier-live-fixture 的 CLI 读取输入后直接 exit 3；gateway-task-lifecycle 不含标记等待循环；w5-states、w6-project、w4-ledger、backfill-ledger 未发现同样的 go/stop 无限等待。未动态验收这些测试，不扩大修复范围。

## 修复说明

定位轮未修改产品代码，也未修改原 logtail-live 测试。仅新增独立回归测试和本文。后续可核对目录消失退出、总寿命上限、等待或终止会话后再删除目录；具体修法由修复轮决定。

## 复核意见

待复核。当前环境未完成预期失败证明，须先在宿主补齐动态复现证据。
