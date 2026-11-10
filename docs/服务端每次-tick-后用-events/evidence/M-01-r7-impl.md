# M-01 · 实现 #7 · 门禁修复受阻

本轮为用户指定的修复轮，不推进新里程碑。起始 HEAD a7d731f，Autome 失败候选 9476ccb512。M-01 原为开放，保持开放；implementation-round 为 7/20。审计文件不存在，无需迁移审计检查。已有项目门禁原样覆盖本次依赖缺失，无新增测试。

## 复现与修复尝试

- 原样执行 `sh scripts/ci-local.sh`，原始输出：`.autome/output/r7-ci-red.txt`。
- package.json、package-lock.json 已声明 ws 8.22.2 与 @types/ws 8.18.2，安装目录缺包；没有需要重复添加的依赖声明。
- 执行 `npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r7-npm-cache"`，退出 1；下载 ws-8.22.2.tgz 时报 `getaddrinfo ENOTFOUND registry.npmjs.org`。日志：`.autome/output/r7-install.txt`。
- `npm cache ls ws` 与 `npm cache ls '@types/ws'` 的结果分别存于 r7-cache.txt、r7-types-cache.txt（均在 .autome/output）。只有其它 ws 版本 tarball 和类型包元数据，没有所需锁定版本的完整缓存，无法离线安装。
- 没有降级依赖、复制 Vite 内部实现、删除类型或跳过失败测试。源代码与锁文件未改；未取得绿色复验，不能声称修复成功。
- `git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0、输出为空。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：检查现有 eventbus 八项用例，覆盖初始化不推历史、2500 条分页、单飞补拉、失败保留游标、重复页去重、hello/序号过滤、订阅退订、关闭；执行结果见下。真实服务五项受缺包阻塞。M-02 resume 情形表不适用本轮依赖修复。
2. **失败分支**：现有用例检查读盘失败后原游标重试、中文日志、空页；Origin 拒绝用例无法收集，未取证。实际执行依赖安装失败分支，记录 DNS 错误。续传超时属 M-02，不适用本轮。
3. **全部迁移**：现有单测覆盖未初始化→初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭（含在途返回丢弃）；真实升级与 SIGTERM 退出受缺包阻塞。resuming/core 广播迁移属 M-02，不适用本轮。
4. **输入域边界**：现有单测覆盖游标 0、MAX_SAFE_INTEGER、空页、单事件、1000 条分页、2500 条跨页、重复序号。非法 resume 及续传 1000/1001 边界属 M-02，不适用本轮。依赖锁定版本保持不变。

## 结论

网络与离线缓存条件不足，修复未完成；这是环境阻塞，不是协议失败。需恢复安装条件后以同一门禁复验，最终以 Autome 在提交后的重跑为准。本会话不启动下一轮。

## 门禁结果与用例基线

`sh scripts/ci-local.sh` 退出 **1**，耗时 **128 秒**。九项通过：Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build。Web typecheck 因缺少 ws 报 TS2307 及派生 TS7006；Web test 的 eventbus-live、gateway-http、server 三个文件导入失败。复现了用户报告的同一失败；安装失败后没有可供绿色复验的新代码或新依赖。

Web 实际收集 **164** 项，全部通过，0 跳过；18 文件中 15 通过、3 收集失败。与上一轮实跑基线 164 **差 0**，无增删测试；相对 M-01 预期 188 缺 **24** 项（server/gateway-http 既有 19 项、eventbus-live 新增 5 项未收集）。eventbus 八项包含在本次通过集合中，支撑上述自审单测结果；164 全过不等于全量通过。

Desktop **104** 项，103 通过、0 失败、1 基线跳过；与上一轮基线 **差 0**。`git diff --check` 通过。仅更新设计状态与证据指针、新增本证据及 retro 一行，未推进或关闭任何里程碑。
