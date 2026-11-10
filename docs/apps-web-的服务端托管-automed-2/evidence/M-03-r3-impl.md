# M-03 · 实现 #3

结论：实现及本里程碑用例通过，申请待审；全量的 7 个既有 server 用例受沙箱监听权限阻塞，按设计「风险与验证安排」第 2 条交审计复验。implementation-round 为 3/25。

## 实现与迁移

- 新增 restart-policy、CoreSupervisor、ticker，保持 5000ms 快速失败阈值、三次失败停机、1000ms 重启延迟、3000ms tick。
- supervisor 用真实 Reply/进程退出判断 tick 是否结束；本地超时带 timeout 标记，不释放在途请求。迟到对账 Reply 才转 connected；手动重启重置失败预算，启动中/运行中不重复启动。
- call 包装协议字段并返回 outcome.payload，内核错误保留 message/code；CALL_TIMEOUT_MS=15000，并交叉检查 Rust 10 秒驱动锁等待。
- 桌面 restart-policy 改成 TS/ESM，增加参数类型及 TS 所需 number 收窄，算法不变。原 5 条测试只把 CommonJS/node:test 注册改为 ESM/vitest，原 assert 断言全部保留；另加 1 条阈值边界测试。
- fake-core 增加 FAKE_CORE_DELAY_MS 作为未指定请求延迟时的默认值，由临时 shell 子进程设置，未修改 process.env。新增 fail-fast fixture。每个真实内核使用 mkdtemp home；未改 desktop、crates、依赖或其它里程碑代码。
- 上轮审计 M-02 通过，无退回缺陷，故无须搬入审计缺陷复现用例。

## 命令、结果与基线差

原始输出均在 .autome/output/，这里仅记录判据：

| 命令 | 结果 | 输出 |
|---|---|---|
| cargo build -p automed | 成功 | m03-build.txt |
| npm run lint --prefix apps/web | 成功，0 warning | m03-lint-final.txt |
| npm run typecheck --prefix apps/web | 成功 | m03-types-final.txt |
| npm test --prefix apps/web | 初跑：137 总数，129 通过、1 失败、7 未执行；常量解析测试遗漏 std::time:: 命名空间已修复；server 初始化遇 EMFILE | m03-web.txt |
| CHOKIDAR_USEPOLLING=true npm test --prefix apps/web | 最终：138 总数，131 通过、7 未执行；1 文件 beforeAll 因 listen EPERM 失败，12 文件通过；退出 1 | m03-web-verified-final.txt |
| npx vitest run test/core-restart.test.ts test/core-supervisor.test.ts test/ticker.test.ts（apps/web） | 25/25 通过；随后补强的 stop 用例亦在最终全量通过 | m03-targeted.txt |
| npm test --prefix apps/desktop | 总数 104，103 通过、1 跳过、0 失败 | m03-desktop.txt |
| git diff --check | 通过 | 无输出 |

与上一轮 Web 113 个基线相比 **+25**：原策略迁移 5 + 边界 1 + supervisor 10 + ticker 9 = 138。设计原预计 +24，额外 1 个是 4999/5000ms 与非有限寿命的边界检查。全部新增 25 个实际通过，无新增跳过。全量减少执行的 7 个是既有 server.test.ts 的 beforeAll 被沙箱禁止监听，不能宣称全量通过。轮询仅绕开 EMFILE 以暴露真正的 EPERM，未修改服务器或跳过条件。desktop 总数差 0，缺 Electron 的 1 个跳过与上一轮一致。

## 标待审之前的自审清单

### 1. 情形表逐行

§4 六行情形均逐行执行（ticker.test.ts）：

1. 未连接：9000ms 零调用，通过。
2. 上次未回复：9000ms 只有一次；释放之后不补排队，下一拍恢复，通过。
3. skipped：日志零调用且 onTick 收到 report；真实 Perl flock 持有驱动锁后内核返回 skipped:true，日志仍零调用，通过。
4. errors 非空：甲/乙逐条形成两行 [调度] 日志，通过。
5. 调用失败：原错误记录一行，下一拍恢复；本地超时但 busy 仍真则后续拍不发送，busy 清除后的下一拍恢复，通过。
6. stop：随后 9000ms 零调用，通过。

### 2. 失败分支

- 真实进程 SIGKILL：进入 restarting，随后新 pid connected，失败计数清零，通过。
- stderr ERROR 后立即退出：三次 starting 后 offline，reason 保留“内核测试启动失败”，不再 spawn，通过。
- 二进制缺失：三次失败后 offline，原因包含“无法启动内核”，宿主测试进程继续，通过。
- 手动重启失败：抛原话；restarting 期间再手动重启也清零预算，无重复定时重启，通过。
- 对账超时：starting/busy 保持，restart 返回 already_running；迟到 Reply 才 connected，通过。
- 普通 call 内核错误：保留 code；call 超时保留 timeout 标记；stopped 时拒绝 call，通过。
- 授权拒绝、外部服务不可达、读取任意文件失败：不适用，本里程碑无鉴权/网络服务/文件读取接口；二进制读取/执行失败已以上述缺失二进制覆盖。

### 3. 全部迁移

- stopped → starting → connected：首次 start 对账通过；onStatus 顺序断言。
- connected → restarting → starting → connected：真实 SIGKILL 与重启通过。
- starting → restarting → starting，第三次 starting → offline：fail-fast 与缺失二进制通过。
- offline → starting → connected：在同一路径修复 executable 后，同一 supervisor.restart 成功。
- restarting → starting：手动失败重试用例通过；计数重新从 1 起。
- starting/connected + restart：already_running；超时对账期间无第二条 tick，通过。
- starting + 本地超时 → starting；starting + 迟到 Reply → connected，通过。
- connected/starting/restarting/offline → stopped：各用例 stop/afterEach 执行；延迟期间 stop 清除定时器，starting 时 stop 使等待 reject 且 busy 清除，通过。
- stopped → starting：迟到 Reply 用例停止后再次 restart，然后停止，确认最终 stopped 且 busy=false。
- tick 空闲 → 在途 → 本地超时仍在途 → Reply 后空闲，以及在途 → stop/退出后空闲，均已执行。单通道 stdio，无其它通道方向。

### 4. 输入域边界

- 寿命 0、4999、5000ms、-1、NaN、Infinity、缺省；5000ms 清零、4999ms 仍计快速失败，通过。
- 连续失败第 1/2/3 次与恢复后的预算清零，通过。
- ticker 2999ms 零次、9000ms 三次；errors 空/单条失败/两条错误、skipped 时忽略 errors，通过。
- 业务日期、路径、空字符串、最大数组等不适用：本模块不定义这些输入域；路径信任门禁属于已闭合 M-02。

## 负向对照与修复检查

临时逐项修改后测试均红，finally 还原源码，再跑最终全量确认 25 个 M-03 用例全绿：

| 变异 | 判别结果 |
|---|---|
| MAX_QUICK_FAILURES 3 → 4 | 连续三次失败检查红 |
| 删除 ticker 两层在途检查 | 上次未返回不叠加检查红 |
| tick 本地等待结束就清空 tickId | 对账超时 busy 检查红 |
| skipped 时仍记录 errors | skipped 安静检查红 |
| stop 不清理重启定时器 | stop 检查红：starting 预期 1，实际 2 |
| CALL_TIMEOUT_MS 15000 → 5000 | Rust DRIVER_LOCK_WAIT 交叉核对红 |

输出：m03-negative-*.txt、m03-negative-summary.txt、m03-negative-stop-recheck.txt。
自审发现 stop 检查最初在定时器创建后切换假时钟，变异未检出；改为启动前安装假时钟且不重复重置计时器，原变异重新执行变红，恢复实现后最终全量变绿。这是测试验证缺口的当轮修复，不是上轮审计退回。

## 剩余验收与交接

审计环境需复验全部 138 个 Web 用例，尤其本轮受 EPERM 阻塞的 7 个 server 用例。M-04/M-05 未推进。未启动下一会话。
