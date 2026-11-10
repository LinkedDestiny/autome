# M-01 · 实现 #12 · 门禁依赖修复受阻

本轮仅修复用户报告的门禁，不推进新里程碑。起始 HEAD e6d0ef7，失败提交 5e06965d27。implementation-round 更新为 12/20；四个里程碑原来均为开放，保持开放。审计文件不存在，无审计临时检查需要搬迁；本次失败检查已经属于项目原有 typecheck 和 Web test。

## 复现与修复尝试

- 原样执行 `sh scripts/ci-local.sh`，输出 `.autome/output/r12-gate-red.txt`。
- `npm ls --prefix apps/web ws @types/ws` 显示空依赖树，输出 `.autome/output/r12-deps.txt`。核对 package.json 与 package-lock.json：两者一致声明 ws 8.22.2、@types/ws 8.18.2；本地未安装。
- `npm cache add ws@8.22.2 --cache "$PWD/.autome/output/r12-npm-cache" --fetch-retries=0 --fetch-timeout=10000` 失败，原始输出 `.autome/output/r12-fetch.txt` 为 `ENOTFOUND registry.npmjs.org`。外层 tail 返回 0 不代表下载成功。
- `npm install --prefix apps/web --offline --ignore-scripts --no-audit --no-fund --logs-dir "$PWD/.autome/output/r12-npm-logs"` 退出 1，输出 `.autome/output/r12-install.txt`：`ENOTCACHED`，锁定的 ws-8.22.2.tgz 未缓存。
- 当前权限禁止申请网络升级。没有修改源码、版本、锁文件或测试；安装没有成功，没有绿色复验，修复未完成。需要后续执行环境安装锁定依赖后原样重跑门禁。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：对照 M-01 八项现有测试：初始化不推历史、2500 条三页、单飞合并补拉、失败保留游标、重复页去重、hello 后递增、订阅退订、关闭丢弃在途结果。结果以本轮全量执行记录为准；真实服务五项因 ws 缺失无法收集。M-02 续传情形表不适用本轮安装修复。
2. **失败分支**：现有测试执行模拟读盘失败与下一次续拉、空页；本轮另实测在线 DNS 失败与离线缓存缺失。Origin 拒绝和服务关闭未取证，受导入失败阻塞；M-02 续传失败分支不适用。
3. **全部迁移**：现有八项测试覆盖未初始化→初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭及在途结果丢弃。真实升级与 SIGTERM 未取证；resuming/core 广播迁移属于 M-02，不适用本轮。
4. **输入域边界**：现有测试检查游标 0、MAX_SAFE_INTEGER 初始化参数、空页、单事件、1000 条分页、2500 条跨页、重复序号。本轮未新增输入域；非法 resume 与 1000/1001 续传边界属于 M-02，不适用。

## 范围与交接

`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0，输出为空。阻塞为网络不可用且锁定依赖无缓存，不是协议失败。最终门禁以 Autome 在提交后的重跑为准，不启动下一会话。

## 门禁结果与用例总数

原样门禁退出 1，用时 167 秒。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 通过；Web typecheck 与 Web test 失败。类型检查报 ws 的 TS2307 和派生 TS7006，Web 的 server、gateway-http、eventbus-live 三个文件导入失败。

Web 总数 18 个文件，15 通过、3 导入失败；收集用例 164 个，164 通过，较第 11 轮基线 164 差 0，没有增删用例。较 M-01 预期 188 少 24，来源为 server/gateway-http 原有 19 个和 eventbus-live 新增 5 个未收集；现有 eventbus 八项均通过。未将未收集的用例算作通过。Desktop 结果见同一完整日志；源码及测试未修改。`git diff --check` 通过。

Desktop 用例总数 104，103 通过、1 基线跳过，较第 11 轮 104 差 0。
