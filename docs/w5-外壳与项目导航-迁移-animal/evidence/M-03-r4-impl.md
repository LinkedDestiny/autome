# M-03 · 实现 #4 · 门禁复现轮

## 范围与结论

本轮按用户最新指令只处理提交 `9b6481155b` 的门禁失败，不推进 M-04。开始 HEAD 为 `b82121d`（Autome 简报提交），工作区干净。M-03 保持待审，implementation-round 为 4/25，五个里程碑状态均未变。

原报告只有 `FAILED Test` 汇总，没有 Rust 失败用例、断言或错误栈。本轮原样执行门禁，Rust Test 通过，未复现原失败；不能据此判断原失败不存在，也不能声称已经修复。当前可实证的失败是不同的 Web 监听限制。没有可支持的产品修复，因此未改源码、测试、脚本或内核，未加 skip、未降低断言。下一次 Autome 若仍报 Rust Test 失败，需要保留该步骤的完整失败段以定位。本轮没有取得全门禁通过证据。

## 命令与结果

- `sh scripts/ci-local.sh > .autome/output/r4-gate-red.txt 2>&1`：退出码 1，706 秒。文件名 red 表示本轮首次复现尝试，不表示复现到了原 Rust 缺陷。
- Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均被脚本判为通过；只有 Web test 失败。
- Rust Test：`cargo test --workspace --all-targets`，1,095 个用例通过（21 + 11 + 345 + 614 + 0 + 81 + 16 + 7），0 failed、0 ignored，步骤耗时 354 秒。上次给出的日志没有 Rust 用例总数，差值无法核实；本轮没有增删测试。独立 End to end 81/81，通过，199 秒；Adapter flags 1/1，通过，6 filtered out。上述重复执行不计入 Rust 唯一用例数，也不代表真实供应商调用验收。
- Web 全量：39 文件、343 用例，313 passed、11 failed、19 skipped（套件初始化失败导致未执行），121.68 秒。相对上一实现轮 343、Autome 上次 343 的总数差均为 **0**。失败集合与上一实现轮记录相同：gateway-http 的全部 11 条报 `listen EPERM 127.0.0.1`；eventbus-live 7 条、notifier-live 3 条、server 9 条未执行；eventbus-live/server 初始化还报 watcher `EMFILE`。不能将未执行记为通过或宣称 skipped=0。
- Desktop：104 个，103 passed、1 skipped、0 failed；与上一轮总数差 **0**，结果分布不变。脚本判通过不代表 104 条全部执行。
- `npm test --prefix apps/web -- test/labels.test.ts test/task-columns.test.ts > .autome/output/r4-m03-target.txt 2>&1`：退出码 0，2 文件、13/13、skipped 0，6.09 秒。相对上一轮 13，差 **0**；无新增或删除测试。
- 最小环境对照：`node --input-type=module -e 'import net from "node:net"; const server = net.createServer(); server.on("error", e => { console.error(e.code, e.message); process.exitCode = 1; }); server.listen(0, "127.0.0.1", () => server.close());'`：退出码 1，同样 `EPERM listen EPERM: operation not permitted 127.0.0.1`。日志 `.autome/output/r4-listen-probe.txt`。不加载项目源码也失败，证明监听阻塞独立于 W5 代码。此环境禁止提权，无法在本轮解除。EMFILE 未单独归因，不声称已修复。

## 标待审之前的自审清单（本轮保持待审，逐项复核）

1. **情形表逐行**：定点测试第 1 条覆盖 14 种形态：draft/queued/queued.resume → todo；active 的 implement/gate → running，await_design_approval/await_merge/await_info/not_a_bug → needs_you；paused/stopped/failed → needs_you；done → done；cancelled → null。另逐一检查全部 22 个 active 节点。labels 第 2、3 条核对需求七段、Bug 五段、完整覆盖和共享合并节点；全部通过。
2. **设计点名的每条失败分支**：labels 第 4 条实跑未知节点、空串、__proto__、toString 的原值回退；task-columns 第 3、6 条实跑未知/空 state 不入列、无节点状态无阶段。授权、读盘、超时、外部服务不可达对本轮纯映射实现不适用，因为函数不访问外部服务；全量中的监听失败另据实记录，未绕过。
3. **全部状态迁移**：不适用内核迁移实现，本里程碑只读映射，不实现状态机。对全部映射载体，task-columns 第 6 条实跑需求 15 节点、Bug 13 节点乘 active.node/paused.resume/queued.resume/stopped.at/failed.at 共 140 组；done/draft/queued/cancelled/unknown 亦覆盖，通过。
4. **输入域边界**：task-columns 第 4 条空数组四列为 0、单 stopped 只计 needs_you；第 3、6 条及 labels 第 4 条覆盖空串、未知值、缺失节点，通过。日期、闰日、跨年、前导零及数值最大值不适用，因为这里没有日期或数值解析。

## 回归与后续

上一审计 #2 为通过，没有以检查抓到并退回的实现缺陷，故无须逐字搬入审计检查。Autome 的原验收命令已经属于项目测试体系，本轮原样跑过，但原 Rust 红未复现，因此没有虚构红绿对照或添加无依据的回归用例。

本轮仅提交证据、状态与一行 retro。门禁最终以 Autome 对提交后版本的复跑为准；本轮不启动下一会话、不推送。
