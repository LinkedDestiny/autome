# M-01 · 实现 #5 · 门禁修复受阻

本轮为修复轮，不推进新里程碑。起始提交 520a824，用户指定门禁失败提交 f0e36c9afd。M-01 起始为开放，本轮保持；implementation-round 写 5/20。审计文件不存在，无审计判别检查需要迁移；报错检查已经在项目 typecheck 与 Web test 中。

## 复现与修复尝试

- 原样执行 `sh scripts/ci-local.sh`，原始输出 `.autome/output/r5-ci-red.txt`；退出 1，119 秒，Web typecheck 与 Web test 失败，其余九项通过。TS2307/TS7006 与三个测试文件收集失败均由 ws 缺失引起。
- 本轮确认 package.json、package-lock.json 已声明并锁定 ws 8.22.2、@types/ws 8.18.2，但两个 node_modules 目录都不存在。上一轮已补齐声明，重复修改源码或声明不能安装缺失包。
- `npm view ws@8.22.2 version --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r5-npm-cache"` 失败，ENOTFOUND registry.npmjs.org。输出 `.autome/output/r5-registry.txt`。
- `npm cache ls` 成功，但没有锁定版本 ws-8.22.2 或 ws-8.18.2 的缓存条目。输出 `.autome/output/r5-cache.txt`。
- `npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r5-npm-cache"` 退出 1，下载 ws-8.22.2.tgz 时 ENOTFOUND。输出 `.autome/output/r5-install.txt`。安装尝试未修改任何已跟踪代码或锁文件，未安装成功。
- 未取得可验证修复，因此无绿色复验；没有降低版本、改写类型、跳过测试或扩大脚本修改范围。依赖安装受环境阻塞，不是协议失败。
- `git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0，输出为空。

## 用例总数与上一轮基线差

Web 实际收集 **164** 个用例，全过、0 跳过；18 个文件中 15 通过、3 收集失败。与上一轮 164 比 **差 0**，本轮未增删用例；相对 M-01 预期 188 缺 24（server/gateway-http 既有 19 项与 eventbus-live 新增 5 项未收集）。164 全过不代表 Web 全量通过。eventbus.test.ts 八项单测全部通过，下面自审中的单测检查均已由本轮门禁执行。

Desktop 总数 **104**，103 通过、1 基线跳过，0 失败；与上一轮 **差 0**。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：M-01 八项单测逐项核对：首次初始化不推历史、2500 条三页、单飞补拉、失败保留游标、重复页去重、新连接 hello 与序号过滤、订阅退订、关闭丢弃在途结果。真实服务五项（合法升级、外站 Origin 拒绝、HMR 共存、命令行事件送达、SIGTERM 退出）受缺包阻塞。M-02 resume 表不适用本轮依赖修复。
2. **失败分支**：现有测试覆盖模拟读盘失败后的中文日志、原游标重试及空页；Origin 拒绝受缺包阻塞。安装失败已实跑：registry DNS 不可达。恢复续传与超时不适用本轮，归 M-02。
3. **全部迁移**：现有单测覆盖未初始化→初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭（含在途）；真实升级与 SIGTERM 未取证。M-02 resuming/core 广播迁移不适用本轮。
4. **输入域边界**：现有单测覆盖游标 0、MAX_SAFE_INTEGER、空页、单事件、1000 条分页、2500 条跨页、重复序号。resume 非法输入与 1000/1001 续传边界不适用本轮，归 M-02；锁定依赖版本未改。

## 结论

依赖安装仍受 registry DNS 和缓存缺包阻塞，M-01 保持开放；不宣称修复完成或门禁通过。下一轮先恢复锁定依赖安装，再重跑同一门禁。最终门禁以 Autome 提交后重跑为准，本会话不启动下一轮。
