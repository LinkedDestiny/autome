# 实现 #13：M-05 Web 门禁测试内核选择修复

## 范围与根因

本轮是宿主门禁修复轮，不推进新里程碑，不改变 M-01 至 M-07 的已审状态。上一审计 #8 通过、#9 无待审项，没有待移植的审计缺陷检查。状态轮数更新为 13/35。

只读检查宿主日志 `/Users/dannie/project/autome-v2/.autome/output/gates/T-84/merge_gate-849cfcebb1f1.log`：110 文件、828 用例，821 通过、7 失败。失败全部来自 `w9-http.test.ts`：confirm、remove、添加页 action、设置保存 action、设置移除 action、报警 action、环境安装/登录。日志明确显示服务选择 `/Users/dannie/.local/bin/autome core`，随后报「autome：未知命令 `core`」并退出 2，导致读取失败或 HTTP 503。不是 schema 拒绝、就绪时序或预算 CLI 环境丢失。

根因是 rebase 引入的 W9 HTTP 夹具首次以 worktree 的 automed 建账本，随后 startServer 没有传 binaryPath，按本任务新增的正常产品查找顺序选中了已安装旧 CLI。修复：w9Kernel 返回首次选择的 binaryPath，HTTP setup 将它传给 startServer，使同一临时账本重启前后使用同一测试内核。保留 AUTOMED_BIN 测试覆盖语义；生产查找顺序、拒绝规则、HTTP 断言及安全头负向对照都不改。

同类检查：枚举 test 下全部 startServer 调用，只有 W9 未显式指定 binaryPath；其余调用已固定测试内核或包装脚本，见 `.autome/output/r13-server-callers.txt`。共享 w9Kernel 的其余六个测试文件全部纳入本轮回归。

## 命令与结果

- 原始失败用例原样执行：`npm run test --prefix apps/web -- test/w9-http.test.ts`。修复前输出 `.autome/output/r13-red.txt`，9 例均在 HTTP 监听处遇到 `listen EPERM: operation not permitted 127.0.0.1`，尚未到宿主的失败断言；启动描述仍证实选择了已安装 CLI。此结果不冒充宿主错误的完整复现。
- 为隔离监听限制，用 `.autome/output/r13-kernel-probe.mjs` 复现同一调用链的内核重启部分：先 w9Kernel 建临时账本并停止，再按服务缺省解析结果启动 CoreSupervisor 并读 project.list。`node .autome/output/r13-kernel-probe.mjs` 退出 **1（红）**，原样复现「未知命令 core」和退出码 2。`node .autome/output/r13-kernel-probe.mjs pinned` 使用修复后夹具返回的 binaryPath，退出 **0（绿）**，真实握手并返回 `project.list {"projects":[]}`。两次分别记录于 `r13-stdio-red.txt`、`r13-stdio-green.txt`，临时 home 均清理。此诊断实验不是新增 Vitest 用例。
- 修复后原命令重跑：`.autome/output/r13-http-after.txt`，9 例仍因本地监听 EPERM 失败；启动描述已全部指向 worktree 的 automed。没有跳过、mock HTTP 或放宽断言，HTTP 端到端绿证据留给宿主。
- `npm run test --prefix apps/web -- test/settings-onboarding-real.test.ts test/env-real.test.ts test/core-binary.test.ts test/core-sidecar.test.ts`：4 文件、**34 例通过**，0 失败、0 跳过；输出 `.autome/output/r13-subset.txt`。
- `npm run test --prefix apps/web -- test/alerts-real.test.ts test/add-project-real.test.ts test/settings-real.test.ts test/project-confirm-real.test.ts`：4 文件、**15 例通过**，0 失败、0 跳过；输出 `.autome/output/r13-fixtures.txt`。
- `git diff --check` 通过。没有运行全量门禁、宿主快检或不相关测试；没有接触真实账本或安装位置，没有改生产代码。

**总数与基线差**：本轮执行 9 文件、58 个唯一 Vitest 用例，49 通过、9 环境失败、0 跳过。没有新增或删除用例；原 W9 HTTP 文件仍为 9，较宿主同文件差 0。相较上一实现 #12 的 71，本轮总数为 -13，来自测试范围改为 W9 共享夹具和 binary/sidecar 回归，不能解释为覆盖减少或新增。stdio 红绿是同一诊断情形的两次运行，另列 1 个实验，不混入 Vitest 总数。未运行宿主其余 770 例，不宣称全量 828 例已绿。

## 四项自审清单

1. **情形表逐行**：M-05 内核查找九种情形全部由 core-binary 通过；W9 首次启动与重新接入同一临时账本的内核选择由 stdio 红绿验证。共享夹具六文件共 22 例通过；HTTP 九例的安全头、合法请求和输入形状仍保留，但本轮被监听权限阻断，不能记为已通过。
2. **失败分支**：旧安装不认识 core 的分支真实复现（红），固定测试内核后真实读请求成功（绿）。sidecar 18 例覆盖不存在的内核、超时、退出、迟到回复等，全部通过。HTTP Host/Origin 拒绝及其合法对照未取证，需宿主复验；未新增授权或远端服务失败分支。
3. **全部迁移**：本轮涉及的夹具启动→停止→同账本重启→读取→停止清理由 stdio 实验实跑；共享夹具真实 onboarding、项目、报警和环境流程 22 例通过。未改变产品状态机、账本迁移或 outdated 迁移，其他状态迁移不适用。
4. **域边界**：core-binary 9 例覆盖空覆盖变量、多 PATH 候选、无候选、缺省/release profile、autome 参数等。共享夹具测试保留超长输入与项目边界检查。本轮没有新增数值/日期输入域，相关边界不适用。

## 交付与待复验

M-05 最新证据指针指向本文件，里程碑已审状态保持不变。本轮没有取得 HTTP 端到端全绿证据，宿主必须重跑 W9 HTTP 九例、快检和合并前门禁；H-01 至 H-03 人工验收不在本轮取证范围。结束会话，不启动下一节点，不推送。
