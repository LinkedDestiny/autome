# M-03 实现 #4

结论：双运行时 SQLite 探针通过，新增测试通过；全量服务测试受当前沙箱阻断，M-03 保持开放，未标待审。implementation-round 为 4/20。

## 实现与审计处理

上一轮审计 M-02 通过，没有退回缺陷或需要逐字迁移的审计检查。新增三个 scripts 文件与 probe:sqlite 命令，使用 Node 标准库；Electron 定位只读 path.txt/dist，不加载 electron 包、不安装或修改 Desktop。通过 git common-dir 定位主检出。

系统与 Electron 均可用，维持采用 node:sqlite、不引入 better-sqlite3 的结论。本机系统 SQLite 实测为 3.53.4，更新设计中的旧值。Electron stderr 的 codesign_util task_name_for_pid 警告原样保存在输出 warnings 字段；检查正常完成且退出 0。

## 命令与结果

- `npm run probe:sqlite --prefix apps/web`：退出 0。系统 Node 26.5.0 / SQLite 3.53.4；Electron 43.2.0 / Node 24.18.0 / SQLite 3.53.1；两者 ok=true、journalMode=wal。原始日志 `.autome/output/m03-probe.txt`。
- `npm test --prefix apps/web`：退出 1，4 文件、32 用例，25 通过、7 跳过；server.test.ts 的 beforeAll 因监听 127.0.0.1 报 EPERM，同时出现文件监听 EMFILE。7 个服务用例因此未执行，不是通过。原始日志 `.autome/output/m03.txt`。
- 与上一轮审计基线 24 相比总数 +8：设计规定的 SQLite 文件库检查 1、定位 4、版本拒绝 1，以及运行时不存在、非零退出 2。未删除或放宽任何旧测试。
- `cd apps/web && npx vitest run test/sqlite-probe.test.ts`：退出 0，8/8 通过，日志 `.autome/output/m03-focused.txt`；最终全量中同一文件也为 8/8。
- `cd apps/web && npx tsc --noEmit`：退出 0，日志 `.autome/output/m03-types.txt`。
- `/opt/homebrew/opt/node@22/bin/node apps/web/scripts/sqlite-check.mjs`：退出 0；22 线文件库检查通过，日志 `.autome/output/m03-node22.txt`。
- `AUTOME_ELECTRON=/usr/bin/false npm run probe:sqlite --prefix apps/web`：退出 1，中文验证失败；日志 `.autome/output/m03-runtime-failure.txt`。
- PATH 前置 `.autome/output/m03-fake-bin/git`，仅让 git common-dir 返回不存在的主检出，清空 AUTOME_ELECTRON 后运行探针：退出 2，中文「未取证：找不到 Electron 二进制」。日志 `.autome/output/m03-missing.txt`。本 worktree 本就没有完整 Electron 二进制，此检查未修改真实 Git 或 Desktop。
- 负向对照：暂时把首次 `new DatabaseSync(file)` 替为 `new DatabaseSync(":memory:")`，专用测试退出 1，WAL 断言发现 memory；日志 `.autome/output/m03-negative-memory.txt`。随后原样恢复脚本并跑最终全量，SQLite 8 项恢复通过。

## 标待审之前的自审清单

1. **情形表逐行**：系统文件库 WAL → 写入 → 关闭 → 重开读回：真实 Node/Electron 均通过；AUTOME_ELECTRON 优先：单测通过；worktree 缺二进制回退主检出且不加载包：单测通过（假包 index.js 一加载即抛错）；worktree 完整优先：单测通过；都没有返回空：单测通过；Electron 版本不符：真实系统 Node 冒充 Electron 被拒绝。脚本与设计结论一致。
2. **失败分支**：运行时不存在返回 false；运行时非零退出返回 false，完整 CLI 退出 1；Electron 版本不符返回 false；找不到 Electron 完整 CLI 退出 2 并报告未取证；非 WAL 库被断言拒绝。均实际执行。设计未要求授权、网络服务或额外读盘失败注入，不适用：脚本只访问本地临时库和现有二进制。
3. **全部迁移**：数据库创建 → WAL → 建表 → 预处理写入 → 关闭 → 重开 → 查询 → 关闭清理，两个真实运行时都执行；定位的覆盖路径 → worktree → 主检出 → 无可用路径分别由定位测试覆盖。无业务状态机，不适用业务状态迁移。
4. **域边界**：空覆盖值按未设置处理；缺 path.txt、缺 dist 二进制、完整布局、路径含当前中文 worktree 名都覆盖；运行时版本匹配由真实 Electron 验证，不匹配由单测验证。数据用单条中文字符串验证绑定与持久化。无日期或数值输入域，闰日/跨年/最大值不适用。

## 剩余事项

在允许本地监听及文件监听的环境执行同一 M-03 验收，取得全量 32/32 通过再标待审。本轮不修改测试来掩盖沙箱限制，不推进 M-04。未执行整仓 ci-local：本轮对应 M-03，整仓门禁由 Autome 调度。
