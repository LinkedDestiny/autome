# M-01 · 实现 #16 · 门禁修复受阻

本轮为修复轮，不推进新里程碑。起始 HEAD 为 `8eff003`，用户报告的门禁提交为 `b1d95ab741`。状态更新为 `16/20`；四个里程碑起始均为开放，保持原状，没有标成待审或已完成。审计文件不存在；本次报错检查已经包含在项目 typecheck 和 Web test 中，无需复制临时审计检查。

## 原样复现与修复尝试

- `sh scripts/ci-local.sh`：退出 1，147 秒，原始日志 `.autome/output/r16-ci-red.txt`。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 通过；Web typecheck 和 Web test 失败，未运行可选 package。
- 错误与宿主一致：`server/eventbus-ws.ts` 和 `test/eventbus-live.test.ts` 找不到 `ws`（TS2307）并产生 TS7006；eventbus-live、gateway-http、server 三个测试文件因 `Cannot find package 'ws'` 无法加载。
- `npm ls --prefix apps/web ws @types/ws`：依赖树为空，原始输出 `.autome/output/r16-deps.txt`。package.json 和 package-lock.json 已声明 `ws@8.22.2`、`@types/ws@8.18.2`，没有缺失声明；本 worktree 的 Web/Desktop 安装目录检索也未找到可复用的独立 ws 包。
- `npm cache ls 'https://registry.npmjs.org/ws' 'https://registry.npmjs.org/@types/ws'`：无缓存记录，原始输出 `.autome/output/r16-cache.txt`。
- `npm cache add ws@8.22.2 --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r16-npm-cache"`：退出 1，`getaddrinfo ENOTFOUND registry.npmjs.org`，原始日志 `.autome/output/r16-fetch.txt`。下载发生在 DNS 阶段失败，不能据此判断锁定版本是否存在。
- 门禁结束后执行 `npm install --prefix apps/web --offline --ignore-scripts --no-audit --no-fund --logs-dir "$PWD/.autome/output/r16-npm-logs"`：退出 1，锁定的 ws tarball 报 `ENOTCACHED`，原始日志 `.autome/output/r16-install-offline.txt`。

安装未成功，没有业务代码、依赖声明、锁文件或测试改动，无法取得修复后绿色证据。没有有效修复，因此未重复整套门禁。当前环境网络受限且不允许权限升级；没有通过改用 Vite 私有打包代码、跳过测试或伪造模块来掩盖错误。

## 用例总数与上一轮差值

Web 共 18 个测试文件：15 通过、3 导入失败；实际收集总数 164，164 通过、0 跳过，与上一轮 164 相比差 **0**。相比 M-01 应有的 188 少 **24**，来自 server/gateway-http 原有 19 项和 eventbus-live 5 项无法收集；不是删除用例。Desktop 总数 104，103 通过、1 基线跳过，与上一轮 104 相比差 **0**。本轮新增、删除用例均为 0。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：现有 eventbus 八项随全量执行通过：首次初始化不推历史；2500 条分三页广播；并发触发合并补拉且单飞；失败保留游标下次续拉；重复页过滤；新连接先 hello 只收更大序号；订阅及退订；关闭停止拉取并丢弃在途结果。真实服务的合格升级、非法 Origin、HMR 共存、跨进程事件、SIGTERM 五项均因导入失败未取证。M-02 续传情形表不适用本轮依赖修复。
2. **失败分支**：读盘失败、中文日志、保留游标重试由现有 eventbus 测试通过；依赖下载 DNS 失败与离线缓存缺失已实际执行。真实服务 Origin 拒绝、SIGTERM 超时检查未取证。M-02 的续传内核不可用分支不适用本轮。
3. **全部迁移**：初始化→就绪、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭及关闭时在途结果丢弃，均由现有八项覆盖并通过。真实升级与进程退出未取证；resuming 和 core 广播属于 M-02，不适用本轮。
4. **输入域边界**：现有八项覆盖游标 0、MAX_SAFE_INTEGER 初始化查询、空页、单事件、1000 条分页上限、2500 条跨页、重复序号。本轮没有新增输入域；非法 resume、续传缺口 1000/1001 属于 M-02，不适用本轮。

## 范围与交接

`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0 且输出为空；安装尝试后工作区仍干净，本轮只修改状态、证据和 retro。环境阻塞不是协议失败。需要在可下载锁定包或具有缓存的环境完成依赖安装，再执行同一门禁；最终以 Autome 提交后重跑为准。本轮结束后不启动下一会话。
