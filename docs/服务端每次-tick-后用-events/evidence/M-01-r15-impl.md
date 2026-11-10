# M-01 · 实现 #15 · 门禁修复受阻

本轮为修复轮，不推进新里程碑。起始 HEAD 为 783cae3，用户报告的门禁提交为 8413be86f4。状态更新为 15/20；四个里程碑原本均为开放，保持不变。审计文件不存在，无需迁移审计临时检查；本次失败检查本来就在项目 typecheck 和 Web test 中。

## 原样复现与修复尝试

- `sh scripts/ci-local.sh` 退出 1，用时 136 秒；日志 `.autome/output/r15-ci-before.txt`。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 通过；Web typecheck、Web test 失败。
- 类型检查报 `Cannot find module 'ws'`（TS2307）及派生的 TS7006。eventbus-live、gateway-http、server 三个测试文件因 `Cannot find package 'ws'` 无法加载，与宿主报告同源。
- `npm ls --prefix apps/web ws @types/ws` 依赖树为空，日志 `.autome/output/r15-deps.txt`；package.json 与锁文件均已声明两项依赖。排序假设为安装未同步、下载网络不可达、锁定版本不可获取；前两项得到实测支持，网络失败发生在解析域名阶段，不能据此判断版本是否可获取。
- `npm cache add ws@8.22.2 --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r15-npm-cache"` 退出 1，日志 `.autome/output/r15-fetch.txt`：`ENOTFOUND registry.npmjs.org`。只尝试获取缓存，未在门禁运行中改变依赖树。
- 门禁结束后执行 `npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r15-npm-cache"`，退出 1；日志 `.autome/output/r15-install.txt`，锁定包 ws-8.22.2.tgz 下载同样遇到 ENOTFOUND。
- `npm cache ls 'https://registry.npmjs.org/ws' 'https://registry.npmjs.org/@types/ws'` 无结果；`npm install --prefix apps/web --offline --ignore-scripts --no-audit --no-fund --logs-dir "$PWD/.autome/output/r15-npm-logs"` 退出 1，日志 `.autome/output/r15-install-offline.txt` 报 ENOTCACHED。

没有安装成功，未改业务代码、依赖版本、锁文件或测试，没有修复后绿色证据。当前权限不允许网络升级，不能完成本轮要求的修复后全绿。未通过跳过用例或更换依赖掩盖失败；没有发生有效修复，故不重复整套门禁。

## 用例总数与基线差值

Web 共 18 个测试文件：15 通过、3 导入失败；实际收集总数 164，164 通过、0 跳过。对上一轮 164 差 0；对 M-01 预期 188 少 24，来自 server/gateway-http 原有 19 项和 eventbus-live 5 项未能收集，不是删除用例。Desktop 总数 104，103 通过、1 基线跳过，对上一轮 104 差 0。本轮新增、删除用例均为 0。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：门禁中 eventbus 的八个现有用例通过，逐项为初始化不推历史、2500 条三页广播、并发触发单飞补拉、失败保留游标后续拉、重复页去重、新连接 hello 后仅收更大序号、订阅与退订、关闭丢弃在途结果。真实服务的合格升级、非法 Origin、HMR 共存、跨进程事件、SIGTERM 五项均被导入失败阻断，未取证。M-02 续传表不适用本轮依赖修复。
2. **失败分支**：模拟读盘失败、中文错误日志、保留游标并重试已由现有测试通过；在线 DNS 失败、离线缓存缺失已实测。Origin 拒绝、SIGTERM 超时检查尚未运行；M-02 续传内核不可用分支不适用。
3. **全部迁移**：现有八项覆盖初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭及关闭时在途结果丢弃，均通过。真实升级和进程退出未取证；resuming 与 core 广播属于 M-02，不适用本轮。
4. **输入域边界**：现有八项覆盖游标 0、MAX_SAFE_INTEGER 初始化查询、空页、单事件、1000 条页上限、2500 条跨页和重复序号。本轮不新增输入域；非法 resume 和续传 1000/1001 边界属于 M-02，不适用本轮。

## 范围与交接

`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0 且输出为空；安装尝试后工作区仍干净，本轮只更新证据、状态和 retro。环境阻塞不是协议失败。下一执行环境需能下载锁定依赖或提供其 npm 缓存，再安装并执行原样门禁；最终结果以 Autome 提交后复跑为准。本轮不启动下一会话。
