# 审计简报缺少本轮证据文件的确切路径

status: 已定位
fix-round: 0/3
复现命令: cargo test --offline -p automed regression_evidence_destination -- --nocapture
根因: 简报只展示角色会话序号和已有证据，未明确本轮证据写入路径；审计会话序号与门禁使用的实现轮号分叉后，按简报序号命名的证据被拒绝。
复核: 待复核

## 现象

任务卡报告：实现 #3 门禁失败、实现 #4 通过后进入审计 #3；审计按 `brief/audit-3.md` 写出 `M-03-r3-audit.md`，内核实际要求 `M-03-r4-audit.md`，因此报「本轮没有留下证据文件」。任务卡中的 T-15 历史运行及手工改名过程未重新执行，本轮以当前代码构造确定性复现。

定位基线：`9901b70cd2622171433669fcc6e6dd1066454af0`。当前目录为单仓库的 Cargo workspace，不是 Autome 多仓库工作区，无需 `repos:` 成员列表。命令从当前 worktree 根目录执行。

## 复现

回归测试位于 `crates/automed/src/brief.rs` 的 `#[cfg(test)]` 模块，统一前缀 `regression_evidence_destination`：

1. `audit_uses_implementation_round`：状态块实现轮号为 4，审计序号为 3，已有 `M-03-r4-impl.md`。调用真实 `guards::check`，确认 `r3-audit.md` 触发 `evidence_missing`，只改成 `r4-audit.md` 后该错误消失；再调用真实 `brief::build`，要求简报包含 `docs/checkout/evidence/M-03-r4-audit.md`，实际失败。
2. `impl_uses_current_session_round`：实现会话为 #4，启动前状态块仍为 #3，要求简报包含 `docs/checkout/evidence/M-03-r4-impl.md`，实际失败。防止修复时简单统一使用旧状态块轮号。
3. `preserves_session_doc_directory`：会话文档目录为 `docs-repo/custom-docs/checkout`，要求审计目的路径保留完整目录前缀，实际失败。

测试直接调用生产简报生成与证据门禁，不使用网络、睡眠或外部模型，也不通过源码字符串匹配判定。首次和复跑均返回 101，3 个用例均在 `missing current evidence destination` 断言失败；不是编译、依赖或命令不可执行造成的失败。

验证原始输出（相对当前 worktree 根目录，运行产物不提交）：

- `.autome/output/evidence-destination-regression.log`
- `.autome/output/evidence-destination-regression-repeat.log`
- `.autome/output/evidence-destination-existing-tests.log`：执行 `cargo test --offline -p automed brief::tests -- --skip regression_evidence_destination`，17 个既有简报测试全部通过。

## 根因分析

- `crates/automed/src/store.rs:1413` 的 `next_round` 按任务与会话种类分别计算序号，因此实现轮和审计轮不保证相等。
- `crates/automed/src/scheduler.rs:2219` 取得角色序号，`write_brief`（同文件第 2455 行）将该序号用于简报文件名、`Inputs.round` 及预算说明，同时单独读取状态块。
- `crates/automed/src/brief.rs:204` 的 `build` 展示角色序号与里程碑；`counterpart`（第 284 行）仅提供既有实现证据的读取路径，没有生成本轮证据写入路径。已有测试只覆盖上一轮证据与目录前缀，没有验证本轮输出文件。
- `crates/automed/src/guards.rs:136` 的 `evidence_file` 根据 `status.impl_round` 匹配 `-r{k}-audit.md` 或 `-r{k}-impl.md`。正负对照说明门禁按实现轮号执行，错误并非门禁错误使用审计序号。
- `protocol/loop-protocol.md:77` 已给出 `M-03-r7-audit.md · 通过 · 审计 #4` 的例子，说明两种序号可不同。通用模板仍要求会话自行拼接路径；简报缺少具体目的路径使误用自身序号成为可能。

后续最小修复应让实现简报和审计简报明确给出本轮完整目的路径，并区分实现会话的当前轮号与审计对应的实现轮号，保留会话可达的 `doc_dir`。本轮未执行完整调度、真实 Agent 会话或全仓库门禁。

## 修复说明

定位轮仅新增回归测试及本报告，未修改产品逻辑。保持红灯供修复轮验证。

## 复核意见

待独立复核。
