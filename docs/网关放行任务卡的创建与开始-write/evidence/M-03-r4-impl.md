# M-03 实现轮 #4

## 结果与改动

M-03 保持开放，未标待审：当前沙箱拒绝监听 127.0.0.1（listen EPERM），无法取得真实 HTTP 验收通过证据。不是协议失败；不增加 reopen。本轮只推进 M-03，不启动后续会话。

仅在 `apps/web/test/gateway-task-card.test.ts` 增加一个测试，按 D-8 构造恰好 65536 UTF-8 字节、超过 60000 UTF-16 码元的正文。样本覆盖 CRLF、制表符、引号、反斜杠、`</script>`、U+0001..U+001F、中文、emoji、组合字符及末尾两空格与两个换行。通过 HTTP 建 Bug 草稿，再经 HTTP task.get 比较字符串、Buffer 和字节数，同时确认任务保持 draft。未改产品实现、Rust 或桌面端。

上一轮 M-02 审计结论为通过，无实现缺陷，故没有需逐字移入并红绿复验的审计检查。只读核对 `dispatch.rs` 的 task.get 使用 `ctx.store.task_brief(task_id)`，及既有 `brief_is_returned_byte_for_byte` 测试，确认设计中的读取源是内核持久化正文；本轮不把源码核对当作真实 HTTP 通过证据。

## 命令与结果

- 按契约执行 `cargo build -p automed && (cd apps/web && npm ci) && (cd apps/web && npx vitest run test/gateway-task-card.test.ts) && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web`，原始日志 `.autome/output/m03-r4-acceptance.txt`。构建和安装成功，目标测试 4/4 因 listen EPERM 失败，后续 && 命令未执行。
- 单独补跑 `npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`，均通过；日志 `.autome/output/m03-r4-lint.txt`、`.autome/output/m03-r4-typecheck.txt`。
- 调整样本前置断言在启动服务前执行后，执行 `(cd apps/web && npx vitest run test/gateway-task-card.test.ts test/write-gate.test.ts test/read-gate.test.ts test/gateway.test.ts)`：75 个，71 通过、4 失败、0 跳过；相对上一轮四文件基线 74 增加 1，来自新增正文读回用例。新用例样本前置断言通过，随后监听 EPERM；不是正文比较失败。日志 `.autome/output/m03-r4-focused.txt`。
- 全量 `npm test --prefix apps/web`：34 文件、319 个用例，285 通过、15 失败、19 跳过（5 文件失败、29 文件通过）。相对上一轮已审计基线 318 增加 1，仅来自本轮新增用例；相对初始基线 B=301 为 B+18，未删用例。15 个失败为 gateway-http 的 11 个与 gateway-task-card 的 4 个，均监听 EPERM；套件初始化另有监听 EPERM 和文件监听 EMFILE，使 19 个用例未执行。无新增条件跳过。日志 `.autome/output/m03-r4-full.txt`。
- 宿主 `sh scripts/ci-local.sh` 由 Autome 后续执行，本轮未执行，不宣称通过。

## 标待审前的四项自审（已执行可运行项，因缺证据不标待审）

1. **情形表逐行**：M-03 无独立情形表，逐项对照 D-8：混排字符、末尾空白、ASCII 补齐、65536 字节、码元数 >60000 均已实现，样本前置断言通过；HTTP create → task.get → 字符串/Buffer/字节数三项判据均已写入，但运行在监听阶段阻塞，持久化读回未取证。
2. **失败分支**：本里程碑点名的负向对照是 trim、CRLF→LF、上限误写 >=65536、截到 60000 码元。已有样本与精确比较可针对这些变异，但本环境无法运行正路径，未注入变异，不把 EPERM 当作负向对照成功。超限 65537 的现有 HTTP 拒绝检查同样阻塞；现有 write-gate 单元中的字节边界与非字符串拒绝检查通过。授权拒绝、读盘故障、超时、外部服务不可达不适用：M-03 没有新增这类分支，不扩大本轮范围。
3. **全部迁移**：本轮仅未创建 → draft（write）→ draft（read），用例对创建及读取状态都有断言，两段均因监听失败未取证。不触发 start/reply 调度，生命周期成功迁移属于 M-04。
4. **域边界**：新增上限本身 65536 的混排正文样本前置检查通过，实际 HTTP 保存未取证；既有单元覆盖 ASCII 65536/65537、中文与 emoji 字节边界并通过。NUL 不在合法正文域内，孤立代理项按设计 K-6 不纳入。日期、闰日、跨年不适用，正文没有日期输入域。
