# M-03 · 实现 #3 · 实现已落地，验收受阻

## 改动与审计输入

上一轮 M-02 审计通过，无退回缺陷，也没有需要逐字移植的缺陷检查。本轮只改网页写门禁与其测试：白名单增加 task.convert，独立校验非空字符串 task_id、feature 方向与可选布尔 start；不修改读门禁、T-37、页面或协议。

## 命令与结果

原始输出均在 `.autome/output/m03-r3-*.txt`。

- `npm ci --prefix apps/web` 成功（install.txt）。
- 改动前 `npm test --prefix apps/web -- test/write-gate.test.ts`：35/35 通过（gate-baseline.txt）。
- 改动后同命令：36/36 通过，0 失败、0 跳过，总数比上一基线增加 1，来自新增的一个 task.convert 测试；该测试内部包含 5 组正例、9 组反例及读通道拒绝断言（gate.txt）。
- 改动前启动 `npm test --prefix apps/web`：eventbus-live 7 个跳过，后续本地服务测试失败，长时间未汇总，终止，退出 130（web-baseline.txt）。未取得全量基线；执行中后来修改了测试文件，因此此日志也不能作为纯净的完整改动前基线。
- 改动后 `npm test --prefix apps/web`：受同一环境限制，未取得完整汇总，终止，退出 130（web.txt）。不能报告全量用例差值，不能把未运行的用例当作通过或跳过。
- 独立本地监听检查：Node net.createServer().listen(0, '127.0.0.1') 报 `EPERM listen EPERM: operation not permitted 127.0.0.1`（bind.txt）。当前沙箱无法完成要求监听本地端口的 Web 验收。
- `cargo test --workspace`：已汇总的 6 个目标为 1081 passed、0 failed、1 ignored；端到端尚有多个用例运行超过 60 秒，主动终止（退出 130，cargo.txt）。这是部分结果，总数 1082，较上一轮完整基线 1207 少 125，原因是本轮未跑完，不能作为用例减少或全绿证据。本轮未改 Rust 测试。
- `sh scripts/ci-local.sh`：Format、Clippy 已通过；在 Rust 测试阶段终止（退出 130，ci.txt），其余阶段未由该次门禁验完。
- `git diff --check` 通过。

本轮没有取得全套验收通过证据，M-03 保持开放，不标待审；需要在允许本地监听的宿主环境重新取得完整 Web 基线和全量门禁。

## 标待审之前的四项自审

1. **情形表逐行**：设计四组正例（省略 start、true、false、512 字符编号）均通过；八组反例（缺编号、空编号、数字编号、缺 kind、bug、字符串 start、数字 start、513 字符编号）均拒绝，并核对精确中文原因。额外单字符编号通过，null start 拒绝。全部 op 正路径测试通过。
2. **失败分支**：本里程碑三条专属参数拒绝分支与通用长度拒绝均已执行；读门禁拒绝 task.convert 已执行。读盘失败、超时、外部服务不可达不适用于纯参数校验函数；全量服务环境失败另列于上方，未冒充通过。
3. **全部迁移**：网关无状态机，状态迁移不适用。通道检查已执行写通道合法输入放行、非法输入拒绝、读通道拒绝；内核原任务取消与新任务 draft/queued 的迁移由 M-01 测试负责，本轮未修改它们，全量 Rust 复验尚未完成。
4. **输入域边界**：task_id 空串、1、512、513 字符已执行；缺失与错误类型已执行；kind 缺失/feature/bug、start 缺失/true/false/字符串/数字/null 已执行。前导零、日期、闰日和跨年不适用，字段不是数字或日期。

补充检查：独立 `npm run lint --prefix apps/web && npm run typecheck --prefix apps/web` 在 lint 阶段长期未输出汇总，主动终止（130）；typecheck 未执行，不声明通过。
