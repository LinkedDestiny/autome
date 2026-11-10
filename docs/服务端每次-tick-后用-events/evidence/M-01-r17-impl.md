# M-01 · 实现 #17 · 门禁修复受阻

本轮为修复轮，不推进新里程碑。起始 HEAD `b3adbcc`；用户报告的门禁提交为 `cad2cabc23`。implementation-round 写为 `17/20`。四个里程碑起始均为开放，保持原状；没有标成待审或已完成。审计文件不存在，报错检查已在项目 typecheck 和 Web test 中，不存在需要搬入的临时审计检查。

## 原样复现与修复尝试

- `sh scripts/ci-local.sh`：退出 1，用时 173 秒；日志 `.autome/output/r17-gate-red.txt`。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 通过；Web typecheck、Web test 失败。未运行可选 package。
- 与用户报告相同：`server/eventbus-ws.ts`、`test/eventbus-live.test.ts` 找不到 `ws`，导致 TS2307 及后续 TS7006；eventbus-live、gateway-http、server 三个测试文件无法导入。
- `npm ls --prefix apps/web ws @types/ws`：依赖树为空，日志 `.autome/output/r17-deps.txt`。已核对 package.json 与锁文件：声明与锁定 tarball 均存在，未发现缺失声明。
- `npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=15000 --cache "$PWD/.autome/output/r17-npm-cache"`：npm 退出 1（npm debug 日志确认；外层 tail 命令返回 0 不作为安装成功证据）。日志 `.autome/output/r17-install.txt`、`.autome/output/r17-install-exit.txt`。下载 `https://registry.npmjs.org/ws/-/ws-8.22.2.tgz` 时 `getaddrinfo ENOTFOUND registry.npmjs.org`，未成功安装。安装尝试发生在门禁的 Rust 阶段，Web 检查开始前已失败结束。
- `scripts/ci-local.sh` 只在 node_modules 目录不存在时安装依赖，已有目录不会自动补装新增包。当前安装环境网络受限且不能升级权限；修改安装条件也不能解除此次 DNS 阻塞。任务只允许为缺少 autome 二进制修改该脚本，故本轮不扩大修改范围。

没有业务代码、依赖声明、锁文件或测试改动；没有可验证的修复，因此没有绿色结果，也未无意义重跑整套门禁。没有跳过测试或替换为 Vite 内部私有实现。最终门禁仍以 Autome 提交后重跑为准。

## 用例总数与上一轮基线差

Web 18 个文件，15 通过、3 导入失败；收集用例总数 164，164 通过、0 跳过，与上一轮 164 相比差 **0**。相比 M-01 预期 188 少 **24**，为 server/gateway-http 原有 19 项和 eventbus-live 5 项未能收集，并非删除测试。Desktop 总数 104，103 通过、1 基线跳过，与上一轮总数 104 相比差 **0**。本轮新增和删除用例均为 0。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：M-01 现有八项单元测试随全量通过：首次初始化不推历史；2500 条三页广播；并发合并补拉且单飞；失败保留游标再续拉；重复页过滤；新连接 hello 后只收更大序号；订阅及退订；关闭停止拉取并丢弃在途结果。合格升级、非法 Origin、HMR 共存、跨进程事件、SIGTERM 五项因导入失败未取证。M-02 续传情形表不适用本轮依赖修复。
2. **失败分支**：读盘失败、中文日志、保留游标重试由现有测试通过；依赖下载 DNS 失败已实跑。真实服务 Origin 拒绝和 SIGTERM 超时检查未取证。M-02 内核不可用时续传不适用本轮。
3. **全部迁移**：初始化→就绪、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭及关闭时丢弃在途结果，现有八项检查通过。真实升级和进程退出未取证；resuming 与 core 广播属于 M-02，不适用本轮。
4. **输入域边界**：现有测试执行了游标 0、MAX_SAFE_INTEGER 初始化查询、空页、单事件、1000 条分页上限、2500 条跨页、重复序号。本轮没有新增输入域；非法 resume 和续传缺口 1000/1001 属于 M-02，不适用本轮。

## 范围与交接

`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0、输出为空。安装后依赖声明与锁文件无差异。仅更新本轮证据、状态和 retro。环境阻塞不是协议失败；需要可下载锁定包或已备缓存的安装环境，完成安装后原样运行门禁。结束本会话，不启动下一会话。
