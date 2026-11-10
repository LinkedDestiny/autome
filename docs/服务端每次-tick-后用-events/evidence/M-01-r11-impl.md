# M-01 · 实现 #11 · 门禁依赖修复受阻

本轮按用户最新要求仅修复门禁，不推进新里程碑。起始 HEAD 16cc671，用户报告的失败提交为 8ffceba61f。四个里程碑实际均为开放，保持原状态；implementation-round 为 11/20。没有审计文件，也没有需要搬迁的审计临时检查：失败检查已在项目原有 typecheck 与 Web test 中。

## 复现与修复尝试

- 原样执行 `sh scripts/ci-local.sh`，完整输出 `.autome/output/r11-ci-red.txt`；结果见下方。
- `npm ls --prefix apps/web ws @types/ws` 显示空依赖树；package.json 与 package-lock.json 已声明 ws 8.22.2、@types/ws 8.18.2，直接原因仍是安装缺失。
- `npm cache add ws@8.22.2 --cache "$PWD/.autome/output/r11-npm-cache" --fetch-retries=0 --fetch-timeout=10000` 下载失败：ENOTFOUND registry.npmjs.org；输出 `.autome/output/r11-fetch.txt`。外层 tail 的退出码不代表 npm 成功。
- `npm install --prefix apps/web --offline --ignore-scripts --no-audit --no-fund --logs-dir "$PWD/.autome/output/r11-npm-logs"` 退出 1：ENOTCACHED ws-8.22.2.tgz；输出 `.autome/output/r11-install-offline.txt`。
- 当前权限不允许申请网络升级。没有改版本、锁文件、源码或测试，也没有跳过失败检查。安装未成功，未取得绿色复验；不能声称完成修复。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：M-01 现有八项单测检查初始化不推历史、2500 条三页、单飞补拉、失败后续拉、重复页去重、hello 后递增、订阅退订、关闭丢弃在途结果。真实服务五项因依赖缺失无法收集；M-02 resume 表不适用本轮依赖安装修复。
2. **失败分支**：现有单测执行模拟读盘失败后保持游标并重试、空页；实际执行在线 DNS 失败和离线缓存缺失检查。Origin 拒绝与服务关闭未取证；续传超时不适用，属于 M-02。
3. **全部迁移**：现有单测检查未初始化→初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭与在途结果丢弃。真实升级和 SIGTERM 未取证；resuming/core 广播迁移不适用，属于 M-02。
4. **输入域边界**：现有单测检查游标 0、MAX_SAFE_INTEGER 初始化参数、空页、单事件、1000 条页、2500 条跨页、重复序号。本轮无新增输入域；非法 resume 与 1000/1001 续传边界不适用，属于 M-02。

## 范围与结论

`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0，输出为空。修复未完成，阻塞为下载不可用且锁定依赖未缓存，不是协议失败。只更新轮次、最新证据指针与下一步，并追加本轮记录，不启动下一会话。最终门禁以 Autome 对本轮提交的重跑为准。

## 门禁结果与用例总数

原样门禁退出 1，用时 165 秒。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 通过；Web typecheck 与 Web test 失败。类型检查仍报 ws 的 TS2307 及派生 TS7006；Web 的 server、gateway-http、eventbus-live 三个文件导入失败。

Web 18 个文件，15 通过、3 导入失败；收集用例总数 164，164 通过、0 跳过，较第 10 轮 164 差 0，没有新增或删除用例。较 M-01 预期 188 少 24，来源为 server/gateway-http 原有 19 个与 eventbus-live 新增 5 个未能收集；eventbus 的 8 项均通过。Desktop 总数 104，103 通过、1 基线跳过，较第 10 轮差 0。未把未收集的用例计作通过。
