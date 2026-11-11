# M-06 实现 #6：V7 示例键值一致性回归

仅推进 M-06。上一轮 M-05 审计通过，没有退回缺陷及必须逐字搬入的审计检查。

## 实现与验收

在 `plugins/autome/tests/verify-cli.sh readme` 增加 V10：先以 V7 的真实 draft 输出验证原 README，再在临时副本中将 `"state": "draft"` 改为 `"state": "pending"`，确认变体确实包含 pending，要求比较退出 1 且差异包含 pending 与 draft。子 shell 隔离 readme 变量，原文件不变；沿用现有只规范化 id、路径、时间的比较器，不新增依赖或创建路径。

- 基线 `sh plugins/autome/tests/verify-cli.sh readme`：退出 0，3/3。
- 修改后同命令：退出 0，4/4；比基线增加 1，来自 V10 永久负向回归，V7–V9 保留。真实 CLI 为 `autome 0.1.0`，使用临时 AUTOME_HOME 并 unset AUTOMED_DB_PATH。
- `sh plugins/autome/tests/run.sh all`：退出 0，59/59，与上一轮审计 59 项差 0。两套合计 63 项，比对应基线 62 项增加 1，无失败、跳过。
- 缺陷注入：复制插件到 `.autome/output/impl6/mutant/autome`，仅在副本比较器 diff 前将两边 state 字符串统一为 ignored。运行副本 verify-cli.sh readme 退出 1，3/4，V10 为 not ok，V7–V9 仍通过；原件运行 4/4。证明新增回归能拦住忽略状态值的退化。这是主动负向对照，不是上一轮审计退回修复，不计入长期用例总数。
- `sh -n plugins/autome/tests/verify-cli.sh`、`git diff --check`：退出 0。

原始输出在 `.autome/output/impl6/`：baseline.txt、readme.txt、all.txt、mutant.txt、gate.txt。临时副本未修改任何真实 README。

## 标待审之前的自审清单

1. **情形表逐行**：M-06 无单独表，按契约逐项执行：真实示例由 V7 接受；只允许 id、路径、时间规范化的既有实现未变，真实临时路径和时间由 V7/V8 验证；pending 变体由 V10 拒绝。键与值仍执行完整 diff，未降级为仅比较键。
2. **失败分支**：设计点名的 pending 错值已执行，比较退出 1，差异含原值和变体值。故意忽略状态值时 V10 失败。授权、超时、读盘与外部服务不可达不适用：本里程碑只添加本地示例比较回归，不改变这些产品分支。
3. **全部迁移**：不适用，本里程碑不引入状态机；draft → pending 是测试数据变异，不是任务状态迁移。合法 draft 与非法 pending 均已执行。
4. **域边界**：执行单字段变异，其余 JSON 保持相同，避免因无关变化误判；先确认原件通过及 pending 替换确实发生，再断言差异。日期、数值、正文长度的域边界不适用，本轮不改变这些输入域及其规范化规则。

## 全仓门禁与最终检查

`sh scripts/ci-local.sh` 退出 1：Rust 1153 次执行通过；Desktop 104 项（103 通过、1 跳过）；Web 225 项（198 通过、11 失败、16 跳过）。总执行数 1482，与上一轮实现基线差 0；1454 通过、11 失败、17 跳过亦无变化。Web 失败仍为沙箱 `listen EPERM: operation not permitted 127.0.0.1`，其余门禁步骤通过，未修改范围外测试或绕过沙箱；全仓门禁不能声明绿色，需允许监听的环境复验。

最终原件 `sh plugins/autome/tests/verify-cli.sh readme` 再跑 4/4，退出 0，日志 final-readme.txt。状态块按协议保持原格式，implementation-round 为 6/40，M-06 仅标待审；M-07/M-08 未推进。
