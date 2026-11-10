# M-01 · 实现 #19 · 门禁复验与环境阻塞

本轮为修复轮，implementation-round 为 19/20，起始 HEAD 为 820e087。用户报告失败提交为 2a5e963b0f。四个里程碑原状态均为开放，本轮保持不变，不推进新里程碑。审计文件不存在，无需搬入临时审计检查；原失败检查本来就在项目 typecheck/Web test 中。

## 命令与结果

- 原样运行 sh scripts/ci-local.sh，日志 .autome/output/r19-ci-red.txt（文件名 red 表示复现尝试，并不意味着历史错误复现）。门禁实际退出 1，用时 158 秒；外层 tail 命令退出 0，不作为门禁退出码。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均通过，仅 Web test 失败。未跑可选 package。
- 修改前独立运行 npm run typecheck --prefix apps/web 已通过，日志 .autome/output/r19-typecheck-red.txt。当前 ws 8.22.2、@types/ws 8.18.2 均存在，符合锁文件；没有复现上一轮 TS2307 和模块导入失败，不能声称本轮取得该错误的红绿修复。
- npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=15000 --cache "$PWD/.autome/output/r19-npm-cache" 退出 0，返回 up to date，日志 .autome/output/r19-install.txt；未改变 package.json 或锁文件。依赖已经就绪的具体外部原因未核实。
- Web test 中 eventbus-live/server 前置启动报 listen EPERM: operation not permitted 127.0.0.1，伴随 watcher EMFILE；gateway-http 的 10 项也报相同监听权限错误。独立 Node net.createServer().listen(0, '127.0.0.1') 检查退出 1、输出 EPERM，日志 .autome/output/r19-listen.txt，证明阻塞不依赖项目服务器代码。
- git diff 30e5d51 --stat -- crates apps/desktop protocol plugins 退出 0、输出为空；安装后 git status --porcelain 为空，随后仅修改本轮三份记录。

## 用例总数与上一轮差值

Web 共 18 个文件、188 个用例：164 通过、10 失败、14 跳过。较上一轮收集到的 164 增加 24，全部来自依赖恢复后可收集的 gateway-http 10 项、server 9 项、eventbus-live 5 项；没有新增或删除测试。14 项跳过来自 server/eventbus-live 的前置服务无法启动，不是主动忽略，也不满足 M-01 零跳过验收。相比设计预期 188 总数差 0。

Desktop 共 104 个用例，103 通过、1 基线跳过，相比上一轮总数差 0。M-01 的 8 个纯单测全过。由于未修改代码、环境权限不变，未重复运行同一全量命令制造第二份相同失败；无法在当前沙箱提供全绿证据。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：执行现有 M-01 八项测试，初始化不推历史、2500 条三页、单飞合并补拉、失败保留游标、重复页去重、hello 后只收更大序号、订阅退订、关闭丢弃在途结果全部通过。真实服务五项（合法升级、外站 Origin、HMR 共存、跨进程 cancel、SIGTERM）均因前置监听失败未取证；M-02 的续传情形表不适用本轮。
2. **失败分支**：读盘失败、中文日志、原游标重试已通过；真实 Origin 拒绝、超时及关闭行为因端口权限未取证。独立监听负向检查确认 EPERM。外部服务不可达、resume 离线分支不适用本轮依赖复验。
3. **全部迁移**：初始化→就绪、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭（含在途结果丢弃）均随纯单测通过。真实升级与退出未取证；resuming/core 广播迁移属 M-02，不适用本轮。
4. **输入域边界**：游标 0、MAX_SAFE_INTEGER 查询、空页、单事件、1000 页上限、2500 跨页与重复序号随单测通过。没有新增输入域；resume 的 1000/1001 缺口与参数边界属于 M-02，不适用本轮。

## 交接

没有源码修复：历史缺依赖问题当前已不存在，剩余失败是执行环境禁止监听。不得通过跳过测试、修改安全守卫或降低断言掩盖阻塞。M-01 保持开放，下一步由允许监听端口的环境执行同一门禁；最终结果以 Autome 在本轮提交上重跑为准。环境阻塞不属于协议失败。本会话结束后不启动下一会话。
