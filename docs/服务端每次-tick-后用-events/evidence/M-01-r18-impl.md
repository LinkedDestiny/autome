# M-01 · 实现 #18 · 门禁修复受阻

本轮是修复轮，implementation-round 为 `18/20`。起始 HEAD `37463ba`，用户报告失败的提交为 `24325b77e1`。四个里程碑起始均为开放，本轮保持原状，不推进新里程碑、不标待审或已完成。审计文件不存在；失败检查已属于项目 typecheck 与 Web test，不存在需逐字搬入的临时审计检查。

## 复现与修复尝试

- 原样执行 `sh scripts/ci-local.sh`，原始日志 `.autome/output/r18-ci-red.txt`；最终结果见下节。
- 独立执行 `npm run typecheck --prefix apps/web`，退出 2，日志 `.autome/output/r18-typecheck-red.txt`：`eventbus-ws.ts` 与 `eventbus-live.test.ts` 的 TS2307（找不到 ws）及失去类型后的 TS7006，复现用户报告。
- 检查安装目录：`apps/web/node_modules/ws` 和 `apps/web/node_modules/@types/ws` 均不存在。package.json 和 package-lock.json 已声明并锁定 ws 8.22.2、@types/ws 8.18.2。`npm cache ls ws` 仅找到 8.21.0、8.22.0、8.22.1 tarball；`npm cache ls '@types/ws'` 仅找到元数据，没有锁定类型包 tarball。输出分别见 `.autome/output/r18-cache.txt`、`r18-types-cache.txt`。
- 执行 `npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=15000 --cache "$PWD/.autome/output/r18-npm-cache"`，退出 1。日志 `.autome/output/r18-install.txt`：下载锁定 ws tarball 时 `getaddrinfo ENOTFOUND registry.npmjs.org`。安装失败，未产生依赖文件差异。

根因链：门禁只在 node_modules 目录不存在时安装；现有目录缺新依赖；补装被当前网络环境阻断。调整 TypeScript 类型或事件代码不能修复模块不存在。任务只允许为 autome 二进制构建修改 ci-local，故没有扩改脚本。没有降级锁定依赖、跳过测试或使用 Vite 私有实现。本轮没有成功修复，不能提供绿色门禁证据；提交后门禁仍以 Autome 重跑为准。

## 验收结果与基线差

完整门禁退出 **1**，用时 167 秒。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 通过；Web typecheck、Web test 失败。未跑可选 package。没有完成修复，因此未重复执行不变输入的全量门禁。

Web：18 个文件中 15 通过、3 导入失败；收集用例总数 **164**，164 通过、0 跳过，与上一轮 164 相比差 **0**。相比 M-01 预期 188 少 **24**，来自 server/gateway-http 原有 19 项和 eventbus-live 5 项导入失败，未能收集，不是删除测试。M-01 八个纯单测随全量通过。Desktop：总数 **104**，103 通过、1 基线跳过，与上一轮总数 104 相比差 **0**。本轮新增和删除用例均为 0。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：核对 M-01 八个现有单测：初始化不推历史、2500 条三页广播、并发合并补拉且单飞、失败保留游标重试、重复页去重、新连接 hello 后只收更大序号、订阅与退订、关闭并丢弃在途结果。执行结果随完整门禁记录。五项真实服务检查（合法升级、非法 Origin、HMR 共存、跨进程事件、SIGTERM）需要 ws，缺失模块时不能取证。M-02 续传表不适用本轮依赖修复。
2. **失败分支**：现有单测覆盖读盘失败、中文日志和原游标重试；另已实跑依赖下载 DNS 失败。真实 Origin 拒绝和进程退出超时检查未取证。M-02 内核不可用时续传不适用本轮。
3. **全部迁移**：现有单测覆盖初始化→就绪、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭及关闭时丢弃在途结果。真实升级和进程关闭未取证；resuming 与 core 广播属于 M-02，不适用本轮。
4. **输入域边界**：现有单测覆盖游标 0、MAX_SAFE_INTEGER 初始化查询、空页、单事件、1000 条页上限、2500 条跨页、重复序号。本轮没有新增输入域；resume 参数与 1000/1001 缺口属于 M-02，不适用本轮。

## 范围与交接

`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0，输出为空。安装尝试后 `git status --porcelain` 为空；随后仅修改本轮证据、设计状态/指针和 retro。环境阻塞不是协议失败。需要可下载锁定包或具备对应 tarball 缓存的环境，安装后再运行同一门禁。本会话结束，不启动下一会话。
