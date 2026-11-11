# 审计简报缺少本轮证据文件的确切路径

status: 已定位
fix-round: 3/3
复现命令: cargo test --offline -p automed regression_evidence_destination -- --nocapture
根因: 简报只展示角色会话序号和已有证据，未明确本轮证据写入路径；审计会话序号与门禁使用的实现轮号分叉后，按简报序号命名的证据被拒绝。
复核: 通过

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

第 3/3 修复轮：保留第 1 轮的最小产品修复，未追加产品代码改动，定位轮回归测试和项目门禁均未修改。宿主在 `710ec946c4d7850ca5fbf5599df6ffaf94365389` 上报告的 End to end 失败本轮未复现；所提供日志只有汇总，没有具体失败用例，不能据此确定新的代码根因。

执行同一组合命令 `( cargo test --offline -p automed regression_evidence_destination -- --nocapture ) && ( sh scripts/ci-local.sh )`：三个回归用例通过，项目门禁耗时 483 秒、退出码 1。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均返回成功；只有 Web test 失败，19 个文件通过、3 个失败，198 个用例通过、11 个失败、16 个跳过。失败日志明确为 `listen EPERM: operation not permitted 127.0.0.1`。Desktop 日志仍有下载 `fetch failed` 引起的跳过，不能视为真实 Electron 验收通过。当前会话禁止提权，未绕过监听限制或弱化测试，完整门禁仍需 Autome 宿主复跑确认。

门禁原始输出：`.autome/output/fix-3-gate.log`。原复现命令另行复验退出码 0，三个用例通过，完整输出：`.autome/output/fix-3-regression.log`。路径均相对当前 worktree 根目录，运行产物不提交。本轮仅更新本 bug 文档，重置为待复核，不启动后续会话。

第 2/3 修复轮：保留上一轮产品修复，不追加产品代码改动。宿主在 `a84378f0d8d8fe0799174731d21ebb44f184d422` 上报告的 Rust Test 失败在本轮未复现；提供的日志末尾没有具体失败用例，不能据此认定新的代码根因。定位轮回归测试完全未改动。

执行同一组合命令 `( cargo test --offline -p automed regression_evidence_destination -- --nocapture ) && ( sh scripts/ci-local.sh )`：三个回归用例通过，项目门禁耗时 572 秒、退出码 1。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 步骤通过，只有 Web test 失败（19 个文件通过、3 个失败；198 个用例通过、11 个失败、16 个跳过）。错误为 `listen EPERM: operation not permitted 127.0.0.1`，同时出现文件监听 `EMFILE`。Desktop 步骤仍存在 Electron 下载 `fetch failed` 导致的跳过，不代表真实 Electron 验收通过。

门禁完整输出：`.autome/output/fix-2-gate.log`；错误摘录：`.autome/output/fix-2-gate-errors.log`。单独再次执行原复现命令，退出码 0、三个用例通过，输出：`.autome/output/fix-2-regression.log`。以上路径均相对当前 worktree 根目录，运行产物不提交。当前受限环境无法确认完整门禁通过，需要 Autome 宿主复跑原组合命令；未修改门禁、测试或运行参数绕过限制。

本轮仅更新本 bug 文档的轮次、待复核状态和验证记录；`git diff --check` 通过。不启动后续会话。

第 1/3 修复轮在 `crates/automed/src/brief.rs` 的本里程碑段落补充本轮证据的完整写入路径。实现角色使用当前会话的 `input.round`，审计角色使用 `status.impl_round`，路径保留 `input.doc_dir`，并明确审计会话序号不用于证据命名。其它角色不增加证据写入要求。

定位轮的回归测试未修改、删除、跳过或弱化。使用原复现命令验证：修复前退出码 101，3 个用例全部失败；修复后退出码 0，3 个用例全部通过。原始输出分别保存在 `.autome/output/fix-1-before.log` 和 `.autome/output/fix-1-regression.log`。

执行配置的项目门禁 `sh scripts/ci-local.sh`，退出码 1，完整日志：`.autome/output/fix-1-gate.log`。Format、Clippy、工作区 Test、End to end、Adapter flags、Desktop、Contract、Web lint/typecheck/build 步骤通过；Web test 为 19 个文件通过、3 个文件失败，198 个用例通过、11 个失败、16 个跳过。失败日志显示本机监听 `127.0.0.1` 被拒绝（`listen EPERM`），实时服务启动还出现文件监听 `EMFILE`，不能宣称门禁全通过。Desktop 步骤虽返回成功，Electron 下载 `fetch failed` 导致真实 DOM 测试跳过，不能作为 Electron 验收通过证明。未修改门禁或测试以绕过环境限制，需宿主在可监听端口的环境复跑。

`git diff --check` 通过。本轮只修改简报生成和本 bug 文档，不创建其它任务文档，不启动后续会话。

## 复核意见

第 2/3 轮独立复核：通过。

- 复现命令复验：`cargo test --offline -p automed regression_evidence_destination` 三个用例全部通过。
- 差异：定位提交 b928132 到 HEAD 对 `crates/` 仅 `brief.rs` +12 行、删除行 0；回归测试未改动、未删除、未跳过、未弱化。第 2/3 修复轮未追加产品代码。
- 根因成立：门禁 `evidence_file` 按 `status.impl_round` 匹配文件名，简报只给角色会话序号。修复在本里程碑段落给出完整路径：实现用 `input.round`，审计用 `status.impl_round`，保留 `doc_dir`，其它角色不受影响。
- 回归测试有意义：审计用例调用真实 `guards::check` 做正负对照并检查真实 `brief::build` 输出，另有实现轮号与目录前缀用例；范围最小。
- 门禁：修复轮记录的唯一失败是 Web test 的 `listen EPERM`/`EMFILE`（沙箱禁止监听端口），与本改动无关；本轮未重跑完整门禁，需宿主在可监听端口的环境复跑。

第 3/3 轮独立复核：通过。

- 复现命令复验：`cargo test --offline -p automed regression_evidence_destination` 三个用例全部通过。
- 差异：定位提交 f65d0b4 到 HEAD 对产品与测试代码只有 `crates/automed/src/brief.rs` +12 行、删除 0 行；回归测试未改动、未删除、未跳过、未弱化。第 3/3 修复轮未追加产品代码，改动范围最小。
- 根因成立：门禁按 `status.impl_round` 匹配证据文件名，简报原先未给出本轮证据路径；修复按角色给出完整路径（实现用当前会话轮号，审计用实现轮号，保留 `doc_dir`）。
- 门禁：修复轮记录唯一失败为 Web test 的 `listen EPERM`（沙箱禁止监听端口），与本改动无关；本轮未重跑完整门禁，需宿主在可监听端口的环境确认。
