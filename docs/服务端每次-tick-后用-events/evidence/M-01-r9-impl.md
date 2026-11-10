# M-01 · 实现 #9 · 门禁依赖修复受阻

本轮按用户指令只修复门禁，不推进新里程碑。起始 HEAD fec1b17；用户报告失败提交 8057584ecd。当前 M-01 实际为开放，保持开放；状态块为 9/20。没有审计文件，无需搬迁审计检查；原有门禁已经覆盖本次依赖加载失败。

## 复现与修复尝试

- 原样执行 sh scripts/ci-local.sh，原始输出：.autome/output/r9-ci-red.txt。
- 核对 package.json、package-lock.json 与 node_modules：声明与锁定版本一致，但 ws 与 @types/ws 安装目录都不存在。eventbus-ws.ts 静态导入 ws，导致依赖 main.ts 的服务测试也无法收集。
- 在线安装：npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r9-npm-cache"。退出 1，ENOTFOUND registry.npmjs.org；输出 .autome/output/r9-install.txt。
- 离线安装：npm install --prefix apps/web --offline --ignore-scripts --no-audit --no-fund --logs-dir "$PWD/.autome/output/r9-npm-logs"。npm 报 ENOTCACHED ws-8.22.2.tgz（外层 tail 命令退出 0 不代表安装成功）；输出 .autome/output/r9-install-offline.txt。缓存清单 .autome/output/r9-cache.txt 仅有旧版 ws tarball。
- 未更改源码、版本或锁文件，未跳过测试。当前权限禁止申请网络升级；没有可用的锁定包，无法完成安装修复，未取得同命令绿色结果。
- git diff 30e5d51 --stat -- crates apps/desktop protocol plugins 退出 0、输出为空。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：读取并核对 M-01 现有八个测试：初始化不推历史、2500 条三页、单飞补拉、失败原游标重试、重复页去重、hello 与递增过滤、订阅退订、关闭丢弃在途结果。随本次门禁执行；真实服务五项因 ws 导入失败未取证。M-02 的 resume 情形表不适用本轮依赖修复。
2. **失败分支**：现有测试实际模拟读盘失败，断言中文日志、游标不动、再次拉取成功；空页由单飞测试执行。外站 Origin 拒绝、真实服务退出未取证，原因是依赖缺失。在线 DNS 与离线缓存失败均实际执行。续传超时不适用本轮，属于 M-02。
3. **全部迁移**：现有单测执行未初始化→初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭与在途返回丢弃；真实升级和 SIGTERM 未取证。resuming/core 广播迁移不适用本轮，属于 M-02。
4. **输入域边界**：现有测试执行游标 0、MAX_SAFE_INTEGER 初始化参数、空页、单事件、1000 条页与 2500 条跨页、重复序号。未新增输入域；非法 resume 与 1000/1001 续传边界不适用本轮，属于 M-02。

## 结论

修复尚未完成，阻塞为执行环境 DNS 与锁定包缓存缺失，不是协议失败。保持全部里程碑原状态，不启动下一会话。最终门禁以 Autome 对提交的重跑为准。

## 门禁结果与用例差

原样门禁退出 1，耗时 138 秒。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 通过；Web typecheck 和 Web test 失败。TS2307 缺少 ws 与派生 TS7006、三个测试文件导入失败，与用户报告一致。

Web 收集总数 164，164 通过、0 跳过；18 个文件中 15 通过、3 收集失败。相对上一轮基线 164 差 0，没有增删测试；相对 M-01 预期 188 少 24，来自 server/gateway-http 既有 19 个与 eventbus-live 的 5 个未收集。八个 eventbus 单测属于通过集合。Desktop 总数 104，103 通过、1 基线跳过，较上一轮差 0。

未安装成功、环境没有改善，因此不重复同一失败门禁并称之为绿色复验。git diff --check 通过；状态块字段齐全，implementation-round 为 9/20；四个里程碑仍为开放。仅更新本轮文档与证据。
