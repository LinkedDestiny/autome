# M-01 · 实现 #8 · 门禁依赖修复受阻

本轮为指定修复轮，不推进新里程碑。起始 HEAD `57ab8f75c97796dd31a0985d07cb763cbdc0e23a`，用户报告的失败提交为 `f97169edfe`。M-01 起始状态为开放，保持开放；implementation-round 更新为 `8/20`。审计文件不存在，因此没有待逐字搬入测试体系的审计检查。本次失败已由原有门禁覆盖，不新增重复测试。

## 复现与修复尝试

1. 原样运行 `sh scripts/ci-local.sh`，完整输出 `.autome/output/impl-8-ci-red.txt`。
2. 核对 `apps/web/package.json`、锁文件与安装目录：声明了 `ws ^8.22.2`、`@types/ws ^8.18.2`，锁文件固定对应版本，但 `node_modules/ws` 不存在。`server/eventbus-ws.ts` 的静态导入使依赖它的服务测试一起无法收集。
3. 在线安装命令：`npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r8-npm-cache"`。退出 1，下载 `ws-8.22.2.tgz` 时 `getaddrinfo ENOTFOUND registry.npmjs.org`。完整输出 `.autome/output/r8-install.txt`。
4. 离线安装命令：`npm install --prefix apps/web --offline --ignore-scripts --no-audit --no-fund --logs-dir "$PWD/.autome/output/r8-npm-logs"`。退出 1，`ENOTCACHED`：没有 `ws-8.22.2.tgz` 缓存。完整输出 `.autome/output/r8-install-offline.txt`。`npm cache ls ws` 的清单存于 `.autome/output/r8-cache.txt`，只有旧版本 tarball。
5. 未改变源码、依赖版本或锁文件，未跳过失败测试。未取得绿色复验，不声称修复完成。安装命令未改变可供验证的依赖，因此没有以同一无变化环境再跑一次门禁冒充修复后验证。
6. `git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0，输出为空。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：对照 M-01 八条单测：初始化不推历史、2500 条三页、单飞补拉、失败后原游标重试、重复页去重、hello 与连接序号过滤、订阅退订、关闭与在途返回丢弃。检查其现有断言并在本次全量执行；真实服务五项因依赖未安装而未取证。M-02 resume 情形表不适用本轮安装修复。
2. **失败分支**：执行现有读盘失败后重试与中文日志检查；空页通过单飞用例执行。外站 Origin 拒绝与真实服务退出未取证，测试导入失败。依赖安装的在线 DNS 失败、离线缓存缺失均已实际执行。续传超时属于后续 M-02，不适用本轮。
3. **全部迁移**：现有八项单测执行未初始化→初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭（在途结果丢弃）。真实升级与 SIGTERM 关闭未取证。resuming 与 core 广播迁移属于后续 M-02，不适用本轮。
4. **输入域边界**：现有用例执行游标 0、MAX_SAFE_INTEGER 初始化参数、空页、单事件、1000 条分页、2500 条跨页、重复序号。没有新增输入域；resume 的非法输入与 1000/1001 续传边界属于后续 M-02，不适用本轮。

## 后续与结论

需要执行环境能下载锁文件指定的包，或提供其完整 npm 缓存，再安装并运行同一 `sh scripts/ci-local.sh`。当前是环境阻塞，不是协议失败；不推进或关闭任何里程碑，不启动下一会话。最终门禁结论以 Autome 在提交后的重跑为准。

## 门禁结果与基线差

`sh scripts/ci-local.sh` 退出 **1**，耗时 **133 秒**。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 共九项通过；Web typecheck 与 Web test 失败。类型检查报缺少 `ws` 的 TS2307 和派生 TS7006；Web test 的 eventbus-live、gateway-http、server 三个文件因导入 ws 失败而无法收集，复现用户提供的同一失败。

Web 实际收集 **164** 个用例，164 通过、0 跳过，18 个文件中 15 通过、3 收集失败。相对上一轮实跑基线 164，**差 0**，没有新增或删除测试；相对 M-01 预期 188 少 **24** 个，来自 server/gateway-http 既有 19 个与 eventbus-live 新增 5 个未收集。eventbus 的八个单测在本次通过集合中。Desktop 总数 **104**，103 通过、1 基线跳过、0 失败，较上一轮 **差 0**。

`git diff --check` 通过；状态块必需字段与四个开放里程碑检查通过。仅修改状态、证据指针、retro 一行，并新增本证据文件。依赖修复未完成，未获得同命令绿色结果。
