# 内核补齐批次（T-95）：教训

复盘 #4，2026-11-14，覆盖复盘 #3 的版本。本任务没有记录到 Autome 指标，下列条目只引用证据文件和本轮复跑的事实，不含指标数字。

复盘 #3 之后的事实：
- 宿主第二次合并前门禁在 `f0c98afe` 上失败（日志在主检出 `.autome/output/gates/T-95/merge_gate-f0c98afe872f.log`）。
  automed 库测试 993 个中 13 个失败：D-1 的 8 个、4 个 scheduler 测试、1 个 store 迁移测试。
- cargo 在 automed 库测试处停下，automed 的 `end_to_end`、`build_rerun`、`prompt_rendering`、`real_cli` 四个目标都没跑。
- 实现 #20（`c53b2b25`）修好了这 13 个，审计 #19 复验通过，宿主快检在 `c53b2b25` 通过。
- 本轮在 HEAD `ab0d2908` 上跑 `cargo test --offline --workspace --all-targets --no-fail-fast`，退出 101
  （输出在不入库的 `.autome/output/retro4-workspace.txt`）。automed 库测试 993 个全过，用时 73 秒；
  `end_to_end` 256 过、2 败：
  - `the_merge_panel_offers_the_merge_when_only_main_has_moved` 单独重跑仍然失败（`end_to_end.rs:4836`，合并后断言
    `available == false`，实际是 `true`）。
  - `legacy_pointer_e2e_restore_failure_keeps_todays_failure` 在全量运行中因 git commit 失败而红，单独重跑通过。
- 下一次合并前门禁至少还会红 1 个。

本版的改动：L-01、L-02、L-03 原样保留；L-04、L-05、L-06 补上第 20 轮和本轮复跑的事实；新增 L-07。

```yaml
- id: L-01
  domain: verification
  symptom: 审计 #9 让 M-09 通过；后来实现 #17（M-16）跑 `cargo test -p automed --lib protocol::` 得到 8 个失败，都是「D-1：eval 目录 `evals/audit-checks-operator-notes/` 不存在」；复盘 #1、#2、#3 复跑均为 64 过、8 败，直到实现 #20 把三个文件补进 `evals.rs` 的嵌入清单（133 → 136）才转绿
  root_cause: M-09 的验收命令只有 `protocol eval protocol/`，读的是磁盘目录，目录确实在；而编译进二进制的 seed 要在 `crates/automed/src/protocol/evals.rs` 的 include_str! 清单里逐个登记 eval 文件，M-04 登记了 A-1，M-09 没登记 D-1；唯一能拦住它的 `--lib protocol::` 只写进了 M-04 的契约，没写进 M-09 的
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-16-r20-impl.md
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
  symptom: 复盘 #1 在 retro.md 写明「HEAD 上 automed --lib protocol:: 8 败（D-1 eval 未进 seed，M-09 引入），合并前门禁会红」，复盘 #2、#3 又写进 lessons.md；此后实现 #18、#19、#20 和审计 #17、#18、#19 共六份简报里，grep「D-1」「protocol::」「复盘」「教训」「lessons」均为 0 命中；D-1 最后是实现 #20 从第二次合并前门禁日志里看到才修的，比复盘 #1 晚了两次门禁
  root_cause: 复盘轮发现的「HEAD 上有已知失败」只落在 retro.md 和 lessons.md，这两份文件不进实现轮、审计轮简报；所有里程碑已完成时，简报的「本里程碑」「上一轮对手轮的结论」两节都是空的，没有别的通道把它送到下一轮
  evidence: docs/内核补齐批次-2026-11-13-4/brief/impl-20.md
  level: brief
  proposal: retro.md 里记了 HEAD 上未解决的测试失败时，之后每一份实现轮简报都必须原文列出这条失败和复现命令，直到某一轮证据写出该命令在当轮 HEAD 上通过
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: tooling
  symptom: 宿主合并前门禁跑 `cargo test --workspace --all-targets`，两次都在第一个失败的测试目标处停下。`2e0696bf` 上停在 autome-domain，automed 一个都没跑，实现 #19 只修了日志里的 7 个；`f0c98afe` 上停在 automed 库测试（13 败），automed `end_to_end` 的 258 个一个都没跑，实现 #20 只修了这 13 个。复盘 #4 在 HEAD `ab0d2908` 上加 `--no-fail-fast` 才看到 `end_to_end` 还有 2 败，其中 1 个能稳定复现。三批失败分属三个目标，被拆成了三次门禁
  root_cause: 项目门禁脚本 `scripts/ci-local.sh` 第 83 行的测试段是 `cargo test --workspace --all-targets -- --nocapture`，没有 `--no-fail-fast`，第一个失败的测试二进制之后的目标全被跳过；门禁修复轮把门禁日志当成失败的全集，没有意识到日志只覆盖到第一个失败的目标
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-16-r20-impl.md
  level: test
  proposal: 项目门禁的 cargo 测试段带 `--no-fail-fast`，合并前门禁日志对 workspace 的每个测试目标都有一行 `test result`
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: verification
  symptom: M-11、M-12、M-14、M-15 各自审计通过，第一次合并前门禁在它们改过的回包形状上红了 7 个（domain 的 `gate_quick_resolved_json_is_unchanged_without_quick`，以及 Web 的 notifier-backfill-real、project-confirm-real、settings-real、settings-onboarding-real、task-detail-real），由实现 #19 专门修。同类的还有一个：M-13 按 F-R3 让已合并任务的 `task.changes` 返回 `merged: true` 结构，审计 #13 通过；基线就有的 e2e `the_merge_panel_offers_the_merge_when_only_main_has_moved` 仍断言合并后 `available == false`，复盘 #4 在 HEAD 上复跑稳定失败
  root_cause: before_merge 模式下实现轮和审计轮只跑「与本轮改动直接相关的测试子集」，选子集都用本轮用例的名字前缀（M-13 用的是 `task_changes_status_`、`events_by_subject_`、`session_ended_`）；断言同一个 RPC 回包的既有测试名字对不上前缀——`end_to_end.rs` 里调用 `"task.changes"` 的有 17 处，`apps/web/test/` 里还有消费这些回包的测试——一次都没跑
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-13-r14-audit.md
  level: rule
  proposal: 里程碑改动了某个 RPC 方法的回包或某个序列化类型的字段时，其相关子集必须包含 `grep -rl` 该方法名或类型名命中的全部测试文件（含 `crates/*/tests/` 与 `apps/web/test/`），证据写出这条 grep 命令和命中清单
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-07
  domain: verification
  symptom: 第二次合并前门禁 automed 库测试的 13 个失败里，除 D-1 的 8 个外，另 5 个都是任务基线就有的单元测试：4 个 scheduler 测试（实现 #20 把它们归到 M-07 与 M-11：M-07 在 `2a391d8c` 改了 `scheduler.rs` 的定位快照守卫，这 4 个从定位后的节点起步、没有定位产物，被新守卫正确拒绝），和 `store::tests::kernel_mark_migration_preserves_queue`（实现 #18 rebase 时把 Onboarding 迁移顺延到 v9）。20 个实现轮证据里的 automed 库测试命令全都带名字过滤，没有一次不带过滤地跑过 `cargo test -p automed --lib`；复盘 #4 不带过滤跑一遍是 993 个、73 秒
  root_cause: 相关子集按本轮新增用例的名字前缀选（M-07 只跑 `--test end_to_end -- bug_regression_base_ bug_regression_guard_ bug_cd_`，实现 #18 只跑 `onboarding` 与 `onboarding_session_migration`），被改动的源文件所在 crate 里的既有单元测试按名字选不到；把整个 crate 的库测试跑一遍只要约一分钟，但没有哪一轮把它算进相关子集
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-16-r20-impl.md
  level: rule
  proposal: 实现轮或审计轮只要改动了 `crates/<c>/src/` 下的文件（包括 rebase 解冲突），证据里就必须有一次不带名字过滤的 `cargo test -p <c> --lib`，并贴出它的 `test result` 行
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
