# M-01 · 实现 #4 · 门禁修复受阻

本轮为修复轮，不推进新里程碑。起始提交 30c198e，M-01 实际状态为开放，全部里程碑状态保持不变；implementation-round 为 4/20。审计文件不存在，无需迁移审计判别检查；报错检查本身已在项目 typecheck 与 Web test 体系内。

## 命令、复现与修复尝试

- 原样执行 `sh scripts/ci-local.sh`：退出 1，131 秒。Web typecheck、Web test 失败，其余九项通过。原始输出：`.autome/output/r4-ci-red.txt`。
- TS2307 指向 eventbus-ws.ts 与 eventbus-live.test.ts 的 ws 导入，TS7006 是类型缺失的连带错误；三个测试文件因找不到 ws 在收集阶段失败。确认 apps/web/node_modules/ws 与 @types/ws 均不存在，但 package.json、package-lock.json 已声明并锁定版本，不能通过再改声明解决。
- `npm view ws@8.22.2 version --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r4-npm-cache"`：npm 失败，ENOTFOUND registry.npmjs.org；原始输出 `.autome/output/r4-registry.txt`。
- `npm cache ls`：退出 0；默认缓存仅列出 ws 8.21.0、8.22.0、8.22.1 的 tarball，未列出锁定的 8.22.2。原始输出 `.autome/output/r4-all-cache.txt`。
- `npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r4-npm-cache"`：退出 1，下载 ws-8.22.2.tgz 时 ENOTFOUND。原始输出 `.autome/output/r4-install.txt`。安装尝试在门禁运行期间执行，但未成功安装依赖，未修改任何已跟踪源码或锁文件。
- 无可验证的修复，因此没有绿色复验；未降低版本、跳过测试或伪造类型声明。项目脚本仅在 node_modules 不存在时安装依赖，当前目录存在，因此继续运行也不会自行补包；任务对脚本修改的授权仅限 CLI 二进制准备，本轮不扩大改动范围。
- `git diff 30e5d51 --stat -- crates apps/desktop protocol plugins`：退出 0，输出为空。

## 用例总数与上一轮基线差

Web 实际收集总数 **164**，全部通过、0 跳过；18 个文件中 15 通过、3 收集失败。与上一轮 164 相比 **差 0**，未增删用例。相对 M-01 预期 188 缺 24：server/gateway-http 的既有 19 项与 eventbus-live 新增 5 项未收集。相对原始基线 175 为 -11（新增单元测试 8 项，缺既有 19 项）。164 全过不代表 Web 全量通过。

Desktop 总数 **104**，103 通过、1 基线跳过、0 失败；与上一轮总数相比 **差 0**。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：门禁执行 eventbus 八项测试，初始化不推历史、2500 条三页、单飞补拉、失败保留游标、重复页去重、新连接 hello 与序号过滤、订阅退订、关闭丢弃在途结果均通过。M-01 的五项真实服务检查因 ws 缺失未收集，均未取证。M-02 resume 情形表不适用本轮依赖修复。
2. **失败分支**：既有测试的模拟读盘失败、中文日志、原游标重试、空页均通过；外站 Origin 拒绝未取证。安装失败分支已实跑，DNS 失败。内核恢复续传及超时不适用本轮，归 M-02。
3. **全部迁移**：既有单测执行未初始化→初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭（含在途），均通过；真实升级与 SIGTERM 关闭未取证。M-02 resuming/core 广播迁移不适用本轮。
4. **输入域边界**：既有单测执行游标 0、MAX_SAFE_INTEGER、空页、单事件、1000 条分页、2500 条跨页、重复序号，均通过。resume 非法输入及 1000/1001 续传边界不适用本轮，归 M-02。依赖锁定版本未变。

## 结论

门禁失败已复现，修复仍被 registry DNS 与缓存缺包阻塞；没有通过证据，M-01 保持开放，不宣称待审或已完成。应先恢复锁定依赖的安装条件，再执行同一门禁。最终结果由 Autome 在本轮提交后重跑判定；本会话不启动下一轮。
