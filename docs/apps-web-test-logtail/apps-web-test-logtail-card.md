# `apps/web/test/logtail-live.test.ts` 每跑一次留下一组不会退出的假会话进程

## 现象

W8（T-59）合并后，每跑一次 Web 测试（包括每次完整门禁 `sh scripts/ci-local.sh`）机器上就多出一组孤儿进程（父进程为 1），永不退出：

```
/bin/sh $TMPDIR/logtail-live-XXXXXX/repo/.autome/skill/run_session.sh ses-…
/bin/sh $TMPDIR/logtail-live-XXXXXX/fake-cli -p
tee -a …/ses-….jsonl
…/target/debug/automed render-stream claude
tee -a …/ses-….log
```

假 CLI 每 50 毫秒 fork 一次 `sleep`。2026-11-13 T-59 开发期间一小时攒了 7 组；T-59 的合并前门禁跑完 1 分钟后又看到新的一组。门禁次数一多，机器负载会被这些进程持续抬高。

## 复现步骤

仓库根目录执行：

```sh
npm --prefix apps/web test -- test/logtail-live.test.ts; sleep 2; pgrep -fl 'logtail-live-[A-Za-z0-9]*/fake-cli'
```

## 期望行为

测试结束（通过、失败、超时都算）后 2 秒内，没有任何命令行里带本次临时目录的进程存活；`pgrep` 无输出。

## 实际行为

`pgrep` 列出 1 个以上 `fake-cli` 进程，以及它所属的 `run_session.sh` / `tee` / `render-stream` 进程，一直存活到被手工杀掉。

## 环境

main @ f5a963ad（2026-11-13，T-59 合并提交），macOS，Node 26。

## 线索

- 假 CLI（测试里 `writeFileSync(fake, …)` 写出的脚本）的收尾循环是 `while [ ! -e "$root/stop" ]; do sleep 0.05; done`。`afterAll` 先写 `go`、`stop` 两个文件，紧接着 `rmSync(root, { recursive: true, force: true })`——目录在假 CLI 下一次轮询之前就没了，`$root/stop` 永远不存在，循环不会结束。测试中途 `rmSync(join(root, 'stop'))`（给第二个会话复位）也会让第一个会话的假 CLI 错过 `stop`。
- 修法方向（修复轮定）：假 CLI 在 `$root` 目录消失时退出（循环条件加 `[ -d "$root" ]`），并给一个总寿命上限；`afterAll` 写完 `stop` 后等会话进程退出（或直接按进程组结束它们）再删目录。两头都做更稳。
- 同目录其它用假 CLI 的测试（`grep -rl "fake-cli\|AUTOMED_CLAUDE_BINARY" apps/web/test`）顺手核对有没有同样的等待循环；有就一并列进 Bug 文档，修不修由复核判断范围。
- **回归测试怎么写（给定位轮）**：不要把断言加在 `logtail-live.test.ts` 自己的 `afterAll` 里——那样修复时必然要改这个文件，会被「不得修改回归测试」挡住。建议新增一个独立的测试文件（例如 `apps/web/test/logtail-live-cleanup.test.ts`），把 `logtail-live.test.ts` 当子进程跑一遍（`npx vitest run test/logtail-live.test.ts`，给足超时），结束后用 `pgrep -f` 或 `ps` 查命令行里带 `logtail-live-` 且启动时间晚于本测试开始的 `fake-cli` 进程，断言为 0，并在 `finally` 里把查到的进程杀掉，避免回归测试自己留垃圾。复现命令就是跑这个新文件。
- 这个新测试要串行、不能和 `logtail-live.test.ts` 本体并发（否则会把正在跑的假 CLI 当成泄漏）：用进程启动时间或子进程的临时目录名来区分，不要只按名字匹配。
- 回归测试提交前跑 `npm --prefix apps/web run lint` 与 `npm --prefix apps/web run typecheck`；复现命令写成仓库根目录可直接执行的形式。
- 只改 `apps/web/test/`（必要时加测试辅助文件），不改产品代码。
