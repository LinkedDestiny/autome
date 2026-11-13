# 内核补齐批次（T-95）：教训

复盘 #3，2026-11-14，覆盖复盘 #2 的版本。本任务没有记录到 Autome 指标，下列条目只引用证据文件和本轮复跑的事实，不含指标数字。

复盘 #2 之后的事实：宿主合并前门禁在 `2e0696bf` 上失败（Rust 1 败、Web 833 个中 6 败，日志在主检出
`.autome/output/gates/T-95/merge_gate-2e0696bfdacd.log`），实现 #19 修了这 7 个，审计 #18 复验通过，
宿主快检在 `84592cb7` 通过。本轮在 HEAD `6dfec533` 复跑 `cargo test --offline -p automed --lib protocol::`：
仍是 64 过、8 败，全是 D-1 eval 目录缺失（输出在不入库的 `.autome/output/retro3-protocol-lib.txt`）。
下一次合并前门禁仍会红。L-01、L-03 原样保留，L-02、L-04 补上第 19 轮的事实，新增 L-05、L-06。

```yaml
- id: L-01
  domain: verification
  symptom: 审计 #9 让 M-09 通过；后来实现 #17（M-16）跑 `cargo test -p automed --lib protocol::` 得到 8 个失败，都是「D-1：eval 目录 `evals/audit-checks-operator-notes/` 不存在」，复盘 #1、#2、#3 复跑均为 64 过、8 败
  root_cause: M-09 的验收命令只有 `protocol eval protocol/`，读的是磁盘目录，目录确实在；而编译进二进制的 seed 要在 `crates/automed/src/protocol/evals.rs` 的 include_str! 清单里逐个登记 eval 文件，M-04 登记了 A-1，M-09 没登记 D-1；唯一能拦住它的 `--lib protocol::` 只写进了 M-04 的契约，没写进 M-09 的
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-16-r17-impl.md
  level: rule
  proposal: 改动 `protocol/CHANGELOG.md` 或 `protocol/evals/` 下文件的里程碑，验收命令必须包含 `cargo test -p automed --lib protocol::`
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: process
  symptom: 实现 #17 和审计 #16 把这 8 个失败记成「既有阻塞」，M-16 照常通过；实现 #18 和审计 #17 沿用「既有」；审计 #18 的契约命令是 `cargo test -p automed -- protocol_integrity_ the_version_in_force`，过滤后同一模块里失败的那 8 个不在运行范围内，8 过即判通过
  root_cause: 「既有」只核对了「把 M-16 的产品改动还原后同样失败」，没有核对失败是在任务基线之前就有，还是本任务提交引入的（D-1 来自 M-09 的提交，rebase 后在 `main..HEAD` 上）；本任务引入的失败没有退回对应里程碑，之后每一轮要么沿用「既有」标签，要么用名字过滤绕开它
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-16-r19-audit.md
  level: protocol
  proposal: 证据把某个测试失败标为「既有」时，必须写出引入它的提交，并说明该提交早于任务基线；引入提交在本任务分支上的，必须把对应里程碑改回 `开放` 并加 reopen
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: verification
  symptom: 审计 #4 和审计 #13 各发现验收契约里有一条负向对照照原文注入后不变红：M-04 把新键写进 `bug-status-block` 契约区，`protocol::` 仍然 71 过；M-13「去掉 `-M`」时，因为 git 默认开启改名检测，用例仍绿
  root_cause: 两个实现轮都是换了注入方式或运行命令才看到红（M-04 改为对 `.autome/output/m04-candidate` 跑静态门，M-13 改用 `--no-renames`），证据里写的是「全部红」或只写了实际做法，没有说契约原文那条不会红，契约的缺口拖到审计才暴露
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-13-r14-audit.md
  level: brief
  proposal: 实现轮执行的负向对照，只要注入内容或运行命令与验收契约原文不同，证据就必须把契约原文和实际做法并排写出，并给出按原文注入后验收命令的实际结果
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: process
  symptom: 复盘 #1 在 retro.md 写明「HEAD 上 automed --lib protocol:: 8 败（D-1 eval 未进 seed，M-09 引入），合并前门禁会红」，复盘 #2 又写进 lessons.md；此后实现 #18、审计 #17、实现 #19、审计 #18 四份简报和实现 #19、审计 #18 两份证据里，grep「D-1」「protocol::」「复盘」「教训」均为 0 命中，四轮都没跑 `--lib protocol::`
  root_cause: 复盘轮发现的「HEAD 上有已知失败」只落在 retro.md 和 lessons.md，这两份文件不进实现轮、审计轮简报；所有里程碑已完成时，简报的「本里程碑」「上一轮对手轮的结论」两节都是空的，没有别的通道把它送到下一轮
  evidence: docs/内核补齐批次-2026-11-13-4/brief/impl-19.md
  level: brief
  proposal: retro.md 里记了 HEAD 上未解决的测试失败时，之后每一份实现轮简报都必须原文列出这条失败和复现命令，直到某一轮证据写出该命令在当轮 HEAD 上通过
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: tooling
  symptom: 宿主合并前门禁在 `2e0696bf` 上跑 `cargo test --workspace --all-targets`，autome-domain 的 1 个断言失败后 cargo 停下，automed 的测试一个都没跑，日志里没有 D-1 的 8 个失败；实现 #19 按日志修了 7 个失败，D-1 原样留着，HEAD `6dfec533` 上仍是 8 败
  root_cause: `scripts/ci-local.sh` 的测试段没有 `--no-fail-fast`，第一个失败的测试二进制之后的目标全被跳过；门禁修复轮把门禁日志当成失败的全集，没有意识到日志只覆盖到第一个失败的 crate
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-16-r19-impl.md
  level: test
  proposal: 项目门禁的 cargo 测试段带 `--no-fail-fast`，合并前门禁日志对 workspace 的每个测试目标都有一行 `test result`
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: verification
  symptom: M-11、M-12、M-14、M-15 各自审计通过；合并前门禁在它们改过的线上形状上红了 7 个：domain 的 `gate_quick_resolved_json_is_unchanged_without_quick`（M-11 加了 `gate_quick_concurrency`），以及 Web 的 notifier-backfill-real、project-confirm-real、settings-real、settings-onboarding-real、task-detail-real，断言的分别是 dashboard 顶层键、`config.get` 全字段、工作区 onboarding 报错和 Bug 文档清单，后来由实现 #19 专门修
  root_cause: before_merge 模式下实现轮和审计轮只跑「与本轮改动直接相关的测试子集」，这四个里程碑选子集都用 Rust 的测试名前缀（`gate_lane_`、`task_documents_`、`config_get_`、`onboarding` 等）；断言同一个类型或同一个 RPC 回包形状的既有测试名字对不上前缀，`apps/web/test/` 里消费这些回包的测试一次都没跑
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-16-r19-impl.md
  level: rule
  proposal: 里程碑改动了某个 RPC 方法的回包或某个序列化类型的字段时，其相关子集必须包含 `grep -rl` 该方法名或类型名命中的全部测试文件（含 `apps/web/test/`），证据写出这条 grep 命令和命中清单
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
