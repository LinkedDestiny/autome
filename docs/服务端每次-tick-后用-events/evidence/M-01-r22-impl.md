# M-01 · 实现 #22 · 宿主门禁失败修复

起始 HEAD：015a082。用户报告的宿主门禁基于 d47ed4dfbd。本轮为修复轮，不推进新里程碑；原表 M-01 为开放，其余也为开放，全部保持原状，implementation-round 更新为 22/30。没有审计文件，因而没有需从上一审计逐字搬入的检查；宿主报错的检查本来就在项目测试体系 test/eventbus-live.test.ts 中，修正后继续作为回归用例。

## 根因与修复

R-02 跨进程 cancel 测试第 78 行误把 payload.state 当成字符串。内核 scheduler.rs 的 apply 路径以 json!({ "state": transition.next, "trigger": ... }) 写事件，TaskState 经带 state 标签的 TaskStateRepr 序列化，真实 events.since 返回 payload 为 {"state":{"state":"cancelled"},"trigger":"cancel"}。原谓词即使收到事件也恒不匹配，最终等到 3500 ms 超时。这与用户提供的宿主失败位置一致。

仅把现有测试的类型断言和读取层级改为 state?.state；不改事件总线、内核载荷、超时、去重或顺序判据。检索 eventbus 系列测试，只有这一处 cancelled 载荷判定。设计文档背景与 §6.2 中的错误扁平载荷同步就地修正；没有向设计追加日志。

## 红绿证据与命令

- 修复前 `sh scripts/ci-local.sh`：退出 1，日志 `.autome/output/r22-ci-red.txt`。除 Web test 外全部步骤通过；Web 18 文件、189 用例，164 过、11 失败、14 跳过。真实网络用例因监听 EPERM（同时有 watcher EMFILE）失败，无法在本沙箱到达宿主第 78 行的超时断言。
- 修复前 `npm test --prefix apps/web -- --run test/eventbus-live.test.ts`：退出 1，5 个用例因 beforeAll 启动失败全部跳过，日志 `.autome/output/r22-live-red.txt`。这是前置失败，不是该断言通过。
- 无端口的真实 CLI 复现：Node 临时仓库、隔离 HOME/AUTOME_HOME、PATH=/usr/bin:/bin，依次 project add --skip-onboarding、task new、cancel T-1、rpc events.since。对真实返回事件执行原谓词（仅去除 TypeScript 类型断言，补 type:event 信封），退出 1、AssertionError；日志 `.autome/output/r22-payload-red.txt` 保留完整载荷与失败表达式。没有启动真实 Agent，临时仓库 finally 删除。
- 对同一份真实账本事件，从修复后的项目测试源码提取谓词并执行：退出 0，匹配 seq=6、task.updated、T-1，额外精确断言整个 payload 等于嵌套状态与 cancel trigger；日志 `.autome/output/r22-payload-green.txt`。这证明字段判定修复，不能替代 WebSocket 延迟验收。
- 可重放命令 `node .autome/output/r22-payload-check.mjs red` / `node .autome/output/r22-payload-check.mjs green` 分别退出 1 / 0，日志 `.autome/output/r22-cli-red.txt` / `.autome/output/r22-cli-green.txt`。脚本分别从起始提交及当前测试文件提取原谓词（仅剥除类型断言），每次独立运行真实 CLI 创建并取消任务、查询 events.since，确认红绿结果可重复且不依赖日志重放。
- 修复后同一命令 `sh scripts/ci-local.sh`：退出 1，164 秒，日志 `.autome/output/r22-ci-green-attempt.txt`。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 全部通过；Web test 仍因监听 EPERM / watcher EMFILE 及其前置超时受阻：18 文件中 15 过、3 失败；189 用例中 164 过、11 失败、14 跳过，与修复前沙箱运行相同。没有获得完整门禁绿证据，真实跨进程送达延迟未取证。
- `git diff --check`：退出 0；`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins`：退出 0、空输出，无禁止目录改动。

## 用例总数与上一轮基线差值

本轮修复前后均未新增、删除或过滤全量用例：Web 总数 189，相比 #21 及用户给出的宿主基线均为 +0；相对设计 M-01 原定 188 多出的 1 个是 #20 的缺少握手字段回 400 回归用例。Desktop 两次总数均为 104，相比 #21 为 +0，103 过、1 原有跳过。CLI 红绿实验是诊断检查，不计入项目用例数。单跑 live 的 5 个用例是全量子集，不重复相加。

## 标待审之前的自审清单

本轮不标待审，仍执行并记录四项核对：

1. **情形表逐行**：M-01 八项 eventbus 单测在修复前门禁全部通过：首次初始化不推历史、2500 条三页广播、单飞合并补拉、失败续拉、重复去重与递增、新连接 hello 与断开、订阅退订、关闭丢弃在途结果。五项 live 情形（合格升级、外站拒绝、HMR 共存、跨进程 cancel、SIGTERM 关闭）被服务启动 EPERM 阻断。cancel 的载荷子判据已取得真实 CLI 红绿证据，网络送达未取证。M-02 的续传表不适用，本轮不推进该里程碑。
2. **失败分支**：假 call 读盘失败保留游标、中文日志和重试通过；错误 Origin、无效握手和缺失内核响应的真实 HTTP 检查受监听限制。原错误载荷谓词失败已复现，修正后对真实载荷通过。外部服务不可达不适用，本任务不调用外部网络服务；前置等待超时不冒充产品失败分支验收。
3. **全部迁移**：初始化→就绪、空闲→拉取→补拉→空闲、失败→成功、连接→断开、订阅→退订、运行→关闭及关闭后在途结果丢弃由八个单测覆盖。真实 CLI failed→cancelled 已执行并核对账本事件；HTTP→WebSocket 与 SIGTERM→连接关闭→进程退出仍未取证。M-02/M-03 的恢复续传及浏览器退避迁移不适用本轮范围。
4. **输入域边界**：单测覆盖 0、MAX_SAFE_INTEGER 初始化游标、空页、单事件、1000 条页上限、2500 条跨页、重复 seq；本次另核对真实取消事件的嵌套对象。错误 Origin、缺握手字段及未注册路径的网络边界未取证。前导零、闰日、跨年、空串不适用，当前改动没有这些输入域。

## 交接

不把字段级红绿实验写成完整门禁通过；M-01 保持开放。最终门禁以 Autome 在本轮提交上重跑为准。未启动下一会话、未推送远端。
