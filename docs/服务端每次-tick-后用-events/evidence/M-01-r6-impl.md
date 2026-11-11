# M-01 · 实现 #6 · 门禁依赖安装受阻

本轮为用户指定的门禁修复轮，不推进里程碑。起始提交 04a6fda，失败候选为 d1b4ae6cc2。M-01 原为开放，保持开放；implementation-round 更新为 6/20。审计文件不存在，没有需迁移的审计检查；本次失败由已有 typecheck 与 Web test 直接守卫。

## 复现与修复尝试

- 原样执行 `sh scripts/ci-local.sh`，完整输出保存在 `.autome/output/r6-ci-red.txt`，结果见下方。
- `apps/web/package.json` 与锁文件已声明 ws 8.22.2、@types/ws 8.18.2；对应 node_modules 目录均不存在。未发现需要重复修改的依赖声明。
- 执行 `npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r6-npm-cache"`，下载 ws-8.22.2.tgz 时失败：`getaddrinfo ENOTFOUND registry.npmjs.org`。完整输出 `.autome/output/r6-install.txt`。未修改依赖声明与锁文件。
- `npm cache ls` 输出存入 `.autome/output/r6-cache.txt`；未找到上述两个锁定版本的 tarball 缓存。无法离线安装，未降级依赖、伪造模块或跳过测试。
- `git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0，输出为空。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：核对 M-01 的八项既有单测：初始化不推历史、2500 条分页、单飞补拉、失败保留游标、重复页去重、hello 与序号过滤、订阅退订、关闭。真实服务五项依赖 ws，安装失败时无法取证。M-02 续传情形表不适用本轮依赖修复。
2. **失败分支**：门禁中的单测检查拉取失败后原游标重试、中文日志和空页；Origin 拒绝检查受缺包阻塞。另实跑依赖下载失败分支并记录 DNS 错误。续传及超时不适用本轮，归 M-02。
3. **全部迁移**：既有单测检查未初始化→初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭。真实升级与 SIGTERM 退出未取证；resuming/core 广播迁移不适用本轮，归 M-02。
4. **输入域边界**：既有单测检查游标 0、MAX_SAFE_INTEGER、空页、单事件、1000 条分页、2500 条跨页、重复序号。非法 resume 与 1000/1001 续传边界不适用本轮，归 M-02。锁定版本保持不变。

## 结论

本轮未取得绿色复验，不能宣称门禁修复完成。安装被 registry DNS 和缓存缺包阻塞；这是环境阻塞，不是协议失败。下一轮需先恢复锁定依赖的安装条件，再执行同一门禁。最终结果以 Autome 对提交的重跑为准；本会话不启动下一轮。

## 门禁最终结果与用例数量

`sh scripts/ci-local.sh` 退出 **1**，耗时 **131 秒**。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 九项通过；Web typecheck 与 Web test 失败。前者是缺少 ws 引起的 TS2307 及派生 TS7006，后者是三个文件导入 ws 失败；与用户给出的失败一致。

Web 实际收集 **164** 个用例，164 通过、0 跳过；18 文件中 15 通过、3 收集失败。与上一轮基线 164 **差 0**，未增删测试。相对 M-01 预期 188 缺少 24 项（server/gateway-http 既有 19 项、eventbus-live 新增 5 项未收集），不能把 164 全过视为全量通过。eventbus 八项单测均通过，上述自审的单测检查由本轮门禁执行。

Desktop **104** 项，103 通过、0 失败、1 基线跳过；与上一轮 **差 0**。`git diff --check` 通过；本轮仅更新设计状态、证据指针、证据文件与 retro 一行。
