# logtail-live 假会话进程未退出
status: 已定位
fix-round: 1/3
复现命令: npm --prefix apps/web test -- test/logtail-live-cleanup.test.ts
根因: afterAll 写入停止标记后立即删除临时目录，假 CLI 错过标记后无限等待，独立会话不会随 Core 退出
复核: 待复核
repos: apps/web

## 现象

任务卡报告：原测试通过后留下 run_session.sh、fake-cli、tee、render-stream 等孤儿进程，假 CLI 每 50ms 创建一次 sleep，重复测试持续积累进程。

期望：测试结束后两秒内，本次临时目录关联的进程全部退出。用户补充的宿主连续两次动态复现已命中泄漏断言，因此状态更新为「已定位」；会话沙箱权限错误不计作复现成功。

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
3. 检查后重跑完全相同的复现命令：退出 1，在启动前报 `spawnSync ps EPERM`，日志 `.autome/output/repro-2.log`。当时尚未得到预期的泄漏断言失败；后续宿主补证见下。
4. quick 所含检查分步执行：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、上述 Web lint/typecheck 均通过；依赖已安装。日志为 `.autome/output/cargo-fmt.log`、`clippy.log`、`lint.log`、`typecheck.log`。没有执行完整门禁。

### 宿主补证与本次核对

用户补充信息（`apps-web-test-logtail-replies.md` 第 1 条，2026-11-13 19:17，macOS、Node v26.5.0）明确记录：准备完成后连续两次执行同一复现命令，均退出 1，失败于第 58 行 `Leaked logtail-live processes after child suite exited: expected [ Array(7) ] to deeply equal []`。没有 listen/spawnSync ps EPERM，也没有超时；每次 finally 后本次临时目录无残留进程。宿主日志位置为主检出 `.autome/output/t86-host-repro-1.log` 和 `.autome/output/t86-host-repro-2.log`。此处依据用户提供的动态结果，未跨 worktree 读取或重新运行这些宿主日志。

本次定位核对基线 `c0e89881`。独立回归测试已由 `48cf58a3` 提交，本次保留其全部内容，不重复创建测试或改动断言。重新执行：

- `npm --prefix apps/web run lint`：退出 0；`npm --prefix apps/web run typecheck`：退出 0。项目无独立 Web 格式化命令，ESLint 通过，无需格式改动。
- 检查后重跑 `npm --prefix apps/web test -- test/logtail-live-cleanup.test.ts`：退出 1，仍在启动子测试前因 `spawnSync ps EPERM` 受阻，不计作预期失败。测试未变，预期失败证据采用上述宿主连续两次结果。
- quick 的 Rust 部分 `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings` 均退出 0；Web 依赖已经存在，lint/typecheck 同上。未运行完整门禁。

本次原始输出：`.autome/output/location-lint.log`、`location-typecheck.log`、`location-repro.log`、`location-fmt.log`、`location-clippy.log`。修复后通过证据仍须由宿主复现检查给出；当前无需补充需求信息。

## 根因分析

宿主已确认残留进程现象；代码核对支持以下机制：

- `apps/web/test/logtail-live.test.ts:50`、`:52` 的两个等待循环仅检查 go/stop 是否存在，没有目录消失退出条件或总寿命上限。
- `:83–89` 的 afterAll 写入 go/stop 后删除整个目录，没有等待假会话进程结束。第三项测试已关闭服务并将 running 设为 undefined；其 afterAll 的可选 close 不会等待运行中的假会话。若 CLI 下一次轮询晚于目录删除，就永远无法再看到标记；甚至可能停在 go 循环。
- `crates/automed/src/launcher.rs:1134` 为会话创建独立进程组，注释明确会话允许比驱动进程活得久。`apps/web/server/core/sidecar.ts:266` 起的 stop 关闭 Core 输入并在必要时结束 Core，不等于结束测试创建的独立会话。测试必须承担自己的进程清理责任。
- 原测试的 logtail.stats 与 lsof 验证覆盖订阅和 Web 进程的句柄，未检查操作系统中的独立会话，因此能漏掉该问题。
- 任务卡提到第二会话复位 stop 可能影响第一会话；但正常路径第二项测试先等待 log.end 和 finished，再进入第三项。因此不能直接断定正常路径的第一会话也会泄漏。本轮优先证据是最后一个仍活动会话与 afterAll 删除标记之间的竞争。

已静态核对同目录引用 fake-cli/AUTOMED_CLAUDE_BINARY 的测试及 fixtures：notifier-live、notifier-live-fixture 的 CLI 读取输入后直接 exit 3；gateway-task-lifecycle 不含标记等待循环；w5-states、w6-project、w4-ledger、backfill-ledger 未发现同样的 go/stop 无限等待。未动态验收这些测试，不扩大修复范围。

## 修复说明

修复第 1/3 轮：仅修改 `apps/web/test/logtail-live.test.ts` 中假 CLI 的 go/stop 等待循环。每次等待前检查所属临时目录是否仍存在，目录消失则 exit 0，使 afterAll 删除目录成为持续可观察的退出条件，避免错过短暂存在的停止标记后无限等待。保留标记正常路径与原有输出；未改变测试断言、输入、用例名或跳过标记。定位轮新增的独立回归测试 `logtail-live-cleanup.test.ts` 完全未修改。未改变 Core 的独立会话生命周期。

本轮验证：

- 原复现命令 `npm --prefix apps/web test -- test/logtail-live-cleanup.test.ts`：退出 1，启动子测试前报 `spawnSync ps EPERM`。属于沙箱限制，未获得修复后通过证据，须由宿主复验。
- 直接相关测试 `npm --prefix apps/web test -- test/logtail-live.test.ts`：退出 1，初始化报 `listen EPERM: operation not permitted 127.0.0.1`，3 项测试跳过，不计通过。
- 临时 Node 验证程序从修改后的文件抽取实际假 CLI 脚本，分别验证等待 go 时删除目录、等待 stop 时删除目录、正常提供 go/stop 三条路径：全部在两秒界限内正常退出；正常路径保留最后一行。该检查只证明假 CLI 退出条件，不替代完整会话进程树回归。
- `npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`、`git diff --check` 均通过。未运行全量门禁 `sh scripts/ci-local.sh`；Rust 未改动，本轮未重跑 Rust 快检，由宿主执行配置的完整快检。

原始日志：`.autome/output/fix-1-repro.log`、`fix-1-live.log`、`fix-1-fixture.log`、`fix-1-lint.log`、`fix-1-typecheck.log`。

## 复核意见

待复核。本轮修复已完成，完整进程树回归通过证据仍待宿主复现检查；后续由 Autome 调度，本轮不启动后续会话。
