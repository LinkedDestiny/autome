# M-01 · 实现 #13 · 门禁修复受阻

本轮为门禁修复轮，不推进新里程碑。起始 HEAD c05bcdc，用户报告的失败提交 8fd342beca。implementation-round 为 13/20；四个里程碑原来均为开放，保持开放。审计文件不存在，无需搬迁审计检查。失败检查本来就在项目 typecheck 与 Web test 中，不另造重复测试。

## 复现与修复尝试

- 原样运行 `sh scripts/ci-local.sh`，完整输出：`.autome/output/r13-ci-red.txt`。
- `npm ls --prefix apps/web ws @types/ws` 返回空依赖树，记录在 `.autome/output/r13-deps.txt`。源码 eventbus-ws.ts 导入 ws，package.json 已声明 ws 与 @types/ws；不是缺少依赖声明。
- 使用任务内空缓存执行离线安装失败（ENOTCACHED），记录在 `.autome/output/r13-install-offline.txt`。
- `npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r13-npm-cache"` 失败：ENOTFOUND registry.npmjs.org，记录在 `.autome/output/r13-install.txt`。外层 tail 的退出码不代表安装成功。
- 使用现有默认缓存运行 `npm install --prefix apps/web --offline --ignore-scripts --no-audit --no-fund --logs-dir "$PWD/.autome/output/r13-npm-logs"`，明确退出 1：锁定的 ws-8.22.2.tgz 未缓存，记录在 `.autome/output/r13-install-default-cache.txt`。
- 当前权限不允许网络升级，无法补齐依赖；无业务代码、测试、依赖版本或锁文件变更。修复尚未完成，无绿色证据，不重复运行未发生变化的全量门禁。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：核对并由全量测试执行 M-01 八项：初始化不推历史、2500 条三页广播、单飞合并补拉、失败保留游标后续拉、重复页去重、新连接 hello 后递增、订阅退订、关闭丢弃在途结果。真实服务五项受 ws 导入失败阻塞；M-02 续传表不适用本轮依赖修复。
2. **失败分支**：现有用例执行模拟读盘失败、中文日志、游标保留与重试；空页由单飞测试覆盖。在线 DNS 失败、离线缓存缺失均实测。外站 Origin 拒绝、服务退出超时判据受导入失败阻塞，未取证。M-02 续传不可用分支不适用本轮。
3. **全部迁移**：现有八项覆盖初始化、空闲到拉取再到补拉与空闲、失败到重试、连接到断开、订阅到退订、运行到关闭（含在途结果）；真实升级和 SIGTERM 关闭未取证。resuming 与 core 广播属于 M-02，不适用。
4. **输入域边界**：现有八项覆盖游标 0、MAX_SAFE_INTEGER 初始化参数、空页、单事件、1000 条页上限、2500 条跨页及重复序号；无新增输入域。非法 resume 和续传 1000/1001 边界属于 M-02，不适用。

## 范围与交接

`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0、输出为空；`git diff --check` 通过。阻塞是依赖下载环境，不是协议失败。最终门禁以 Autome 在本轮提交上重跑为准，本轮不启动下一会话。

## 门禁结果与用例差值

原样命令退出 1，用时 180 秒。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 均通过；Web typecheck、Web test 失败。类型检查报 ws 缺失的 TS2307 及其派生 TS7006，Web 的 server、gateway-http、eventbus-live 三个文件导入失败，与报告的故障一致。

Web 共 18 个文件，15 通过、3 导入失败；实际收集 164 个用例，164 通过、0 跳过。相对第 12 轮收集总数 164 差 0；相对 M-01 预期 188 少 24，来自 server/gateway-http 的原有 19 项与 eventbus-live 的 5 项未被收集，不是删除测试或通过验收。eventbus 的 8 项均通过。Desktop 总数 104，103 通过、1 基线跳过，相对第 12 轮 104 差 0。没有新增或删除测试。
