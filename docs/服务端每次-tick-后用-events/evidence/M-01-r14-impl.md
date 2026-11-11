# M-01 · 实现 #14 · 门禁依赖阻塞

本轮为修复轮，不推进新里程碑。起始 HEAD 为 25aeb83，用户报告失败提交为 8d4e72639c。implementation-round 更新为 14/20，四个里程碑均保持开放。审计文件不存在；本次失败检查已在项目 typecheck 与 Web test 中，不新增重复测试。

## 复现与修复尝试

- 原样执行 `sh scripts/ci-local.sh`，原始输出在 `.autome/output/r14-ci-red.txt`。
- `npm ls --prefix apps/web ws @types/ws` 确认依赖树为空（`.autome/output/r14-deps.txt`）；package.json 和锁文件已声明依赖，eventbus-ws.ts 的导入与设计一致。
- `npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r14-npm-cache"` 安装失败，日志 `.autome/output/r14-install.txt` 报 `ENOTFOUND registry.npmjs.org`，目标为锁定的 ws-8.22.2.tgz。外层 tail 的退出 0 不是安装成功。
- `npm install --prefix apps/web --offline --ignore-scripts --no-audit --no-fund --logs-dir "$PWD/.autome/output/r14-npm-logs"` 退出 1，日志 `.autome/output/r14-install-offline.txt` 报 `ENOTCACHED`，默认缓存也没有该包。
- 当前权限禁止网络升级，无法完成依赖安装。未改变业务代码、依赖版本、锁文件或测试；无修复后绿色证据。不能通过跳过用例、替换依赖或弱化类型检查消除这一环境阻塞。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：M-01 已有八项分别检查初始化不推历史、2500 条三页广播、单飞合并补拉、失败后从原游标续拉、重复页去重、新连接 hello 后递增、订阅退订、关闭丢弃在途结果。真实服务五项依赖 ws，无法加载；M-02 续传情形表不适用本轮依赖修复。
2. **失败分支**：现有测试检查模拟读盘失败、中文日志、游标保留与下次重试；在线 DNS 失败、离线缓存缺失均实测。Origin 拒绝和 SIGTERM 退出超时检查受导入失败阻塞，未取证。M-02 续传的内核不可用分支不适用。
3. **全部迁移**：八项测试覆盖初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭及在途结果丢弃。真实升级、SIGTERM 关闭未取证；resuming 与 core 广播属于 M-02，不适用本轮。
4. **输入域边界**：八项测试覆盖游标 0、MAX_SAFE_INTEGER 初始化参数、空页、单事件、1000 条页上限、2500 条跨页与重复序号。本轮无新增输入域；非法 resume 与续传 1000/1001 边界属于 M-02，不适用。

## 范围与交接

`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0、输出为空。阻塞是依赖安装环境，不是协议失败。下一步须在可访问 npm registry 的环境安装锁定依赖，再原样执行门禁；没有绿色结果前不能标待审。最终门禁以 Autome 在提交后重跑为准，本轮不启动下一个会话。

## 门禁结果与用例差值

原样门禁退出 1，用时 135 秒。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 通过；Web typecheck 与 Web test 失败。类型检查为缺少 ws 的 TS2307 及派生 TS7006；server、gateway-http、eventbus-live 三个测试文件导入失败，与用户报告一致。

Web 共 18 个文件，15 通过、3 导入失败；实际收集总数 164，164 通过、0 跳过，相对上一轮基线 164 差 0。相对 M-01 预期 188 少 24，来自 server/gateway-http 原有 19 项与 eventbus-live 5 项没有被收集，并非删减用例。eventbus 的八项全部通过，对应上述自审清单中的本轮已执行检查。Desktop 总数 104，103 通过、1 基线跳过，与上一轮 104 差 0；没有新增或删除测试。`git diff --check` 通过。
