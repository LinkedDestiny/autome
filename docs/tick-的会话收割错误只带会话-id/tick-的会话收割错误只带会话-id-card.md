# tick 的会话收割错误只带会话 id，不带任务 id：`autome run` / `run --json` 的错误行看不出是哪个任务出的错

## 现象

内核一次 tick 里收割会话失败时，报告的错误只用会话 id 做前缀：

```
{"error":"ses-e77840629ec14d9096128e577a28bc5e: /…/repo/.autome/config.toml 格式错误：TOML parse error at line 1, column 12 …"}
```

操作者从这一行看不出是哪个任务（要拿会话 id 去账本里反查）。同一次 tick 里任务内核步骤（门禁等）的错误是用任务 id 做前缀的，两种错误的定位方式不一致。

这也是 T-46 的用例 `run_json_tick_error_and_interrupt_have_no_idle_trailer` 时序性失败的来源：它断言第一条错误含任务 id；整理会话若在第一次 tick 前已经退出，第一条错误来自会话收割（只有 `ses-…`），断言失败。2026-11-13 这条用例在 T-50 的合并前门禁里失败过一次（`.autome/output/gates/T-50/merge_gate-7fd7bf81bff5.log` 第 88–113 行），单跑则通过。

## 复现步骤

1. 新建项目与任务（`task new`，假 CLI 的整理会话会很快退出），**等整理会话的退出标记写完**；
2. 把任务状态改成 `Active { node: Gate }`，把项目 `.autome/config.toml` 写成非法 TOML（`invalid = [`）；
3. `autome run --json --interval 0.05`，读第一行输出。

（T-72 的定位轮已经把上面的时序固定成确定性的写法：在 `crates/autome-cli/tests/cli.rs` 的同名用例里、改状态之前，有界等待账本里运行中会话的退出文件能被 `ExitMarker::parse` 完整解析。该提交是分支 `autome/时序性失败的测试-run-json-tick` 上的 `639fa30 test(cli): 固定 tick 错误首行竞争的复现时序`，可以直接参照或 cherry-pick 作为本任务的回归测试。）

## 期望行为

**产品决定（操作者代用户给出）：tick 报告的每一条错误都必须能直接看出任务。**

- 会话收割失败的错误前缀同时含任务 id 与会话 id，任务 id 在前，例如 `T-1 ses-…: <原因>`；任务内核步骤的错误格式不变（仍以任务 id 开头）。
- `autome run --json` 的 `{"error": …}` 行与 `autome run` 的可读错误行都体现这一点；输出顺序不重排、不去重。
- 上面复现步骤里第一行错误包含任务 id；`run --json` 的其它保证不变：SIGINT 后正常退出、stderr 为空、没有空闲尾行。
- 找不到会话所属任务的极端情况（账本不一致）退回只带会话 id，不 panic。

## 实际行为

`crates/automed/src/scheduler.rs` 的 tick 先收割会话（约 173–188 行），收割出错时用会话 id 包装错误；随后才推进任务内核步骤（约 195–219 行），那里的错误用任务 id 包装。`crates/autome-cli/src/main.rs`（约 855–861 行）按报告顺序逐条输出。

## 环境

main `d9a8cc1`；`crates/automed/src/scheduler.rs`（tick 的错误包装）、`crates/autome-cli/src/main.rs`、`crates/autome-cli/tests/cli.rs`。

## 线索（可选）

- **回归测试的写法决定这个任务能不能走完**：定位轮写的回归测试必须是「等会话退出后，断言第一条错误含任务 id」这种**对产品行为的断言**，修复轮改产品代码让它变绿。不要把它定位成「测试本身有缺陷」——Bug Loop 的修复轮不允许改回归测试，T-72 就是这样三轮修复全部卡住后被取消的（它的定位与分析见分支 `autome/时序性失败的测试-run-json-tick` 的 `docs/时序性失败的测试-run-json-tick/时序性失败的测试-run-json-tick-bug.md`，根因链路可以直接引用）。
- 其它断言了旧前缀 `ses-…:` 的测试或金样，按新格式更新（这是产品格式变更的必要跟随，不算弱化）。
- 复现命令用仓库根目录可直接执行的形式：`cargo test --offline -p autome-cli --test cli run_json_tick_error`。
