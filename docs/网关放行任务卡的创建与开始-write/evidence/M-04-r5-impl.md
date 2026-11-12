# M-04 实现 #5

结论：未通过，M-04 保持开放；本轮预算 5/20。新增两条测试，仅修改 apps/web 测试与任务记录，未修改产品代码、crates 或 Desktop。上一轮审计 #3 通过 M-03，没有退回缺陷或必须逐字迁入的审计检查。

## 实现

新增 `apps/web/test/gateway-task-lifecycle.test.ts` 两条真实 HTTP 网关测试。临时项目使用既有 configureFailureRepo 禁止重试；从 bug-protocol.md 解析状态块生成无法复现文档。启动服务前同时断言 Claude/Codex 替身路径，替身记录自身路径、提交定位文档。通过显式 tick 和 20 秒有界轮询等待 await_info；首轮与补充后的第二轮分别要求 history 恰好包含 1/2 条替身路径。清理按服务关闭、环境恢复、临时目录删除顺序执行。

## 命令与结果

- `cargo build -p automed`：通过，`.autome/output/m04-build.txt`。
- `npm ci --prefix apps/web`：通过，`.autome/output/m04-ci.txt`。
- `(cd apps/web && npx vitest run test/gateway-task-lifecycle.test.ts test/gateway-task-card.test.ts)`：6 条全部失败；均为 HTTP 监听 `127.0.0.1` 的 `listen EPERM: operation not permitted`，业务断言未执行。日志 `.autome/output/m04-target.txt`。
- `npm run lint --prefix apps/web`：通过，日志 `.autome/output/m04-lint.txt`。
- `npm run typecheck --prefix apps/web`：通过，日志 `.autome/output/m04-typecheck.txt`。
- 全量结果见下文补记。

定向用例总数 6，较上一轮 M-03 定向 4 增加 2，均来自新 lifecycle 文件；没有删除、跳过或弱化既有用例。全量上一轮基线 319（原始 B=301），本轮预期 321（B+20）。监听权限由宿主约束，不能将此失败描述为产品缺陷或协议失败，也不能把替身 history 非空、补充逐字落盘描述成已经取证。

## 标待审之前的自审清单（本轮未标待审）

1. **情形表逐行**：D-10 开始草稿 → 200、queued/active → await_info → history 一条；补充空白 → 409 中文；合法补充 → 200、原文保存、第二轮 await_info → history 两条。两条测试已覆盖这些判据，但本次执行全部被监听 EPERM 阻断，未取证。
2. **失败分支**：设计点名的空白补充分支另加空串检查，并断言 task.state 不变、replies 文件未产生；已运行但被夹具监听失败阻断。拒绝授权、读盘失败、外部服务不可达不适用：D-10 未设计这些产品分支；20 秒轮询超时会失败，未制造超时负向对照。四项变异负向对照未运行：当前环境正向也不能到达断言，无法获得有效判别证据。
3. **全部迁移**：本里程碑范围为 draft → queued/active → locate 完成 → await_info；await_info + 空/空白补充 → 原状态；await_info + 有效补充 → 新 locate → await_info。测试覆盖上述路径并检查 1/2 次替身执行，运行未取证。其它 Bug 状态迁移不适用，本轮未改变内核状态机。
4. **域边界**：新增检查 info 空串、纯空白、前导与内部双空格、中文及结尾换行逐字保存；运行未取证。info 的缺参、非字符串和 8000/8001 边界由已有 write-gate 测试维护，本轮不删改。前导零、闰日、跨年不适用，本里程碑没有数字或日期输入域。

## 后续

在允许本地 HTTP 监听的运行环境执行 M-04 完整验收命令，核实两轮替身 history 与 replies 原文，再进行负向对照、自审并报待审。宿主 `sh scripts/ci-local.sh` 由 Autome 执行，本轮未运行，不能代称通过。

## 全量与补充验证

- `npm test --prefix apps/web`：共 **321** 条，285 通过、17 失败、19 因套件初始化失败而未执行（vitest 报 skipped）；35 文件中 29 通过、6 失败。较上一轮实际基线 **319 增加 2**，恰为本轮新增 lifecycle 用例；没有删减用例。失败涉及监听 EPERM、实时服务启动超时及文件监听 EMFILE。原始输出 `.autome/output/m04-all.txt`，不能视为 A-05 通过。
- `(cd apps/web && npx vitest run test/write-gate.test.ts test/read-gate.test.ts test/gateway.test.ts)`：**71/71 通过**，其中已有 info 缺参、类型及 8000 边界守卫通过；日志 `.autome/output/m04-gates.txt`。这只证明门禁与透传单元层，不证明实际会话。
- `git diff --check`：通过。
