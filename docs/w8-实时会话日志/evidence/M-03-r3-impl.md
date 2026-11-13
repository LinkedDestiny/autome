# M-03 实现 #3

结论：待审（按 M-03 验收契约的 EPERM 环境例外提交复验；端到端未取证，不宣称通过）。implementation-round：3/25。

## 改动与接口核对

- `server/main.ts` 创建并返回 logtail，关闭时在 notifier 后、bus 前等待 logtail.close。
- 新增 `test/logtail-live.test.ts` 三条测试：同源 /ws 收取随机 marker 的延迟小于 3000ms；结束尾行、连续字节偏移、全文相等及 lsof 正向对照；客户端路径拒绝和活跃订阅关闭释放。
- 临时 HOME / AUTOME_HOME 与两个 CLI 路径隔离；包装脚本启动真实 automed，不修改 process.env；先构建 production 资源。假 CLI 由 go/stop 文件控制，afterAll 即使断言失败也释放触发条件。
- 源码核对 `dispatch.rs::task_get`：sessions 位于回包顶层；按真实形状读取。任务标题带随机串，避免第二个任务复用 slug。
- 关闭测试在 bus.close 入口前断言资源归零，防止 bus 的 disconnect 清理掩盖漏调 logtail.close。未修改总线、安全检查、内核或前端。
- 上轮审计结论通过，无待搬入的缺陷检查，因此本轮没有审计缺陷红绿复现。

## 命令与结果

| 命令 | 退出码 | 结果 / 原始输出 |
|---|---|---|
| `cargo build -p automed -p autome-cli` | 0 | `.autome/output/m03-build.txt` |
| `npm test --prefix apps/web -- test/logtail-live.test.ts test/core-sidecar.test.ts` | 1 | 21 条：18 通过，3 因 beforeAll 的 listen EPERM 未执行；`.autome/output/m03-tests-final.txt` |
| `npm test --prefix apps/web -- test/logtail.test.ts test/eventbus.test.ts` | 0 | 43 条全部通过；`.autome/output/m03-unit.txt` |
| `npm run lint --prefix apps/web` | 0 | `.autome/output/m03-lint.txt` |
| `npm run typecheck --prefix apps/web` | 0 | `.autome/output/m03-types.txt` |
| Node net.createServer 监听 127.0.0.1:0 探针 | 1 | EPERM；`.autome/output/m03-listen.txt` |
| `git diff --check` | 0 | 无空白错误 |

typecheck 首次发现测试变量 running 可能 undefined，改用已捕获的 service 后通过。两次验收尝试均在监听阶段被环境阻止，未绕过、未添加条件 skip。beforeAll 内 npm build 已完成，未取得实时耗时或 lsof 运行证据。全量门禁和宿主快检未运行。

用例总数：本轮去重共 **64** 条（21 验收 + 43 直接相关回归），61 通过、3 环境阻塞。上一轮审计范围为 43 条，本轮差 **+21**：新增 live 3 条，纳入已有 core-sidecar 18 条；不是新增 21 条测试。M-03 自身相对 core-sidecar 的 18 条基线新增 **3** 条，达到契约发现数 21，尚未达到 21 条全部通过。既有 43 条数量不变。

## 标待审之前的自审清单

1. **情形表逐行**
   - M-03 第 1 行：真实任务 running → offset 0 初始日志 → go marker；测试已具备严格 3000ms 断言和耗时记录，实际执行被 EPERM 阻塞。500ms 调度由 logtail 第 21 条通过验证，不能代替真实链路耗时。
   - M-03 第 2 行：stop → scheduler.tick 收割 → finished 尾行 → end → stats/lsof；已编入测试，真实链路未实跑。logtail 第 6、7、13 条尾段及资源释放通过。
   - M-03 第 3 行：path 拒绝 → 新活跃会话 → 服务关闭；已编入测试且检查 bus 关闭之前的资源状态，真实链路未实跑。logtail 第 2、13 条拒绝和关闭通过。
2. **失败分支**
   - 客户端路径越权：logtail 第 2 条通过；live 的真实路径拒绝待复验。
   - 读盘 open/stat/read 错误：第 18 条通过；缺文件、删除、轮转：第 8、9 条通过。
   - 内核离线、not_found、其它错误与恢复：第 10、14 条通过；core-sidecar 超时/退出等既有 18 条全部通过。
   - 空历史：第 7 条通过。外部服务不可达：不适用，本任务仅使用本机内核和文件；本机监听拒绝已单独探针复现并记录。
3. **全部迁移**
   - 无订阅 → 订阅、重复替换、限额拒绝、退订释放：第 1、4、12 条通过。
   - running → 追加；状态查询失败 → 保持跟随 → 恢复结束：第 14、21 条通过。
   - running/历史 → ended；缺文件等待 → 文件出现；deleted/rotated/error → 清理：第 6–9、18 条通过。
   - 活跃/在途 → 退订、断连、替换、close；断连后续传：第 11–13、15、19 条通过。
   - service.close → logtail.close → bus.close 的真实服务迁移待 live 第 3 条复验；未用单元结果代替端到端证据。
4. **输入域边界**
   - offset 缺省、0、超长，空文件，150 KiB 多字节、1 MiB 多轮预算，残缺 UTF-8：第 1、5、7、8、16、17、20 条通过。
   - 第 4/5 个订阅和连接隔离：第 4 条通过。非法 id/额外路径字段：第 2 条通过。
   - 本轮 main 接线不新增输入域；日期、前导零等不适用。真实 session id、中文、随机 marker 与 3000ms 时限边界在 live 中断言，受环境阻塞未实跑。

## 审计下一步

在允许监听的环境执行 M-03 验收命令，取得 21/21、marker 实测耗时及 lsof 正向/负向证据，并执行契约的三项负向对照。本轮负向变体未运行，因为监听失败无法区分变体；不计作通过。依据设计风险表与 M-03 EPERM 条款交待审，不推进 M-04。
