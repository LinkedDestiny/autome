# M-01 r1 审计复验

结论：**实现缺陷**（验收命令未通过）。领域 launcher。

## 验收命令

`cd apps/web && npx vitest run test/web-instance.test.ts test/web-launcher.test.ts test/web-stop-gate.test.ts test/core-supervisor.test.ts`
本机可监听、可执行 ps，全部实跑：**4 个文件、40 条，36 过、4 败**，失败全在 `web-stop-gate.test.ts`。
`sh plugins/autome/tests/run.sh card panel`：29/29 通过。
原始输出：`.autome/output/a1.log`（本轮审计目录，不进仓库）。

## 失败

- 门禁未结束时 stop --wait 2、释放门禁后 run 收取结果、无残留：`TypeError: Cannot read properties of undefined (reading 'pid')`，`test/web-stop-gate.test.ts:22`（`processes(...).find(row => row.command.includes('/gate.sh'))!.pid`）。
- stop --force 停服务但门禁继续存活：`pid argument must be of type number. Received undefined`（`gatePid` 未赋值的连带失败）。
- 套件级：`expect.poll() must be called inside a test`，`test/fixtures/web-launcher.ts:49`（`cleanup()` 在 `afterAll` 里调用 `expect.poll`）。

## 根因（判别实验，临时补丁已还原，git 干净）

setup 只轮询 `/api/instance` 的 `gates`（任务进入 merge_gate 即为真），随即找 `gate.sh` 进程。把查找推迟 8 秒，`gate.sh` 与 `w7-sleep` 进程出现（`task.get` 状态 `merge_gate/active`）；立即查则找不到。即门禁子进程比任务状态晚起，测试在竞态窗口里取 pid。属测试写法缺陷，加之 fixture 清理在测试体外调 `expect.poll`；产品的 stop/gate 提示逻辑本轮没有被证明错，但验收用例不能稳定通过，A-2 的门禁分支（web-stop-gate 第 1 条）因此没有证据。
修法交实现轮：setup 里轮询进程出现再取 pid；`cleanup` 改用普通轮询，不用 `expect.poll`。

## 负向对照（实跑）

- 陈旧判定不核对 `instance`（删去 `result.instance === value.instance`）：web-instance 失败 2 条（换主人、三态表）。✔ 有效
- 「无响应」当「陈旧」（`'unresponsive'`→`'stale'`）：web-instance 失败 2 条（三态表、SIGSTOP/SIGCONT 不接管）。✔ 有效
- 其余负向对照未逐项注入；web-launcher 12 条、core-supervisor 基线、web-instance 14 条在原实现下全过。
- 注入均已还原，`git status` 干净。

## 判别检查留存

无需另存脚本：复现即验收命令中的 `test/web-stop-gate.test.ts`，当前应红，修复后应绿。
