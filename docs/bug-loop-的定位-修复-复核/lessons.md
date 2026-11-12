# 教训 · Bug Loop 的定位 / 修复 / 复核 prompt 列出任务附件与 `--doc` 参考（T-41）

复盘轮 #2 覆盖写（复盘 #1 之后又跑了 M-03：rebase main、协议版本顺延为 v4、实现 #4 + 审计 #3）。依据：任务文件、
`retro.md`、`evidence/` 下 7 份证据、评审 / 裁决 / 审计记录、`.autome/output/r4-conflicts.txt`、`git log`，以及本轮在
HEAD（`ab750c9`）上的一次复跑。本任务没有 Autome 指标，下文不引用任何指标数值。

**本轮复跑发现 HEAD 上有一个确定性失败的测试。**
`cargo test -p automed --test end_to_end every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task`
退出码 101：`end_to_end.rs:1734` 断言 `starts_with("protocol/v3@")`，实际值是 `protocol/v4@5d67a6c…`
（原始输出：`.autome/output/retro2-e2e-protocol-version.txt`）。M-01 的实现 #1 坏的是同一行（当时是 v2→v3），
那次被每轮宿主门禁拦下，多花了实现 #2。这次是 M-03 的 rebase 把版本号顺延到 v4，实现 #4 改了 `protocol/mod.rs`
的断言，没改这一行。M-03 的验收只跑 `end_to_end bug_loop` 过滤子集；审计 #3 写的全量结果是「仅 scheduler 时序用例
失败，其余全过」。但那个用例在 automed 的 lib 单测里，cargo 默认在第一个失败的测试二进制处停下，`end_to_end`
根本没跑到（L-01、L-02、L-05）。任务状态已是「已完成」，合并前门禁会在这里报红。本轮按约束不改代码和里程碑表。

L-03、L-04 与复盘 #1 相同，proposal 原文不动，以便跨任务计数。

```yaml
- id: L-01
  domain: verification
  symptom: '同一类破坏出现了两次。M-01 实现 #1 的验收只跑 launcher / prompt_rendering / protocol:: 和静态门，宿主门禁在 3e839ce 上报 end_to_end 失败（仍断言 protocol/v2@），实现 #2 只改了一行断言。M-03 实现 #4 写「未跑 cargo test --workspace 和 ci-local.sh；本轮只验收冲突关联子集，全量门禁由合并前宿主执行」，契约里的 e2e 也只有 `end_to_end bug_loop` 过滤；rebase 把协议顺延到 v4 后，同一个 e2e 用例在 HEAD 上确定性失败（复盘 #2 复跑退出码 101），里程碑却已关闭'
  root_cause: '改协议版本号是全仓库可见的行为变化，但里程碑验收契约只列了自己直接改的那几个测试目标，用例过滤（bug_loop）把断言版本号的 e2e 用例排除在外；全量运行被推给另一个里程碑或合并前门禁，实现轮自己的验收看不到这次破坏'
  evidence: 'docs/bug-loop-的定位-修复-复核/evidence/M-03-r4-impl.md'
  level: rule
  proposal: '凡改动产品代码的里程碑，其验收契约的验收命令必须包含 `cargo test --workspace`，不得把全量运行只放在单独的「全量门禁」里程碑'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: '`crates/automed/tests/end_to_end.rs` 的 every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task 在同一行已经有 `assert_eq!(protocol_ref, automed::protocol::current().0.to_wire())`，下一行又写死了 `starts_with("protocol/vN@")`。本任务两次升版都撞上这一行：v2→v3（实现 #2 专门改它）和 v3→v4（M-03 漏改，HEAD 上失败）。同一类写死版本号的问题还有 eval.rs 夹具：基线固定追加 `## protocol/v3`，与真实新版本重名。本任务改成 v999，main 上的 T-61 独立改成 v90，两边改法不同，rebase 时 eval.rs 成为 6 个冲突文件之一（.autome/output/r4-conflicts.txt）'
  root_cause: '断言当前版本的测试不止 protocol/mod.rs 一处，e2e 用例里还有一份写死的字面量；每次升版都要人记得同步改它，而升版可能发生在设计轮、实现轮或 rebase 顺延中的任何一处'
  evidence: 'docs/bug-loop-的定位-修复-复核/evidence/M-01-r2-impl.md'
  level: test
  proposal: '`git grep -nE ''"protocol/v[0-9]'' -- crates/automed/tests` 输出为空：`crates/automed/tests/` 下需要当前协议版本的断言一律取 `automed::protocol::current()`，写死的当前版本号只保留在 `protocol/mod.rs` 的那一条单测里'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: protocol-format
  symptom: '实现 #3 写「e2e 实现完成……M-02 保持开放，不标待审」（代码已落地，bug_loop 15/15、workspace 1202 通过，只有 ci-local 的 Desktop 与 Web test 被环境阻断），证据里附有不依赖应用代码的独立探针：Electron `--version` 的 spawnSync status=null、signal=SIGABRT（m02-electron-probe.txt），Node net.Server 监听 127.0.0.1 返回 EPERM（m02-listen-probe.txt）；随后审计 #2 采信宿主在 df489780fe 上的门禁（退出码 0）把同一里程碑直接复验关闭。同样的环境阻断下，实现 #2 却把 M-01 保留为待审，两轮处理不一致'
  root_cause: '协议没有规定「代码已落地、只有验收被执行环境阻断」时里程碑应标什么状态，实现轮各自理解；而宿主门禁每个实现轮后都会在允许 Electron 与回环监听的环境里重跑，审计也采信它'
  evidence: 'docs/bug-loop-的定位-修复-复核/evidence/M-02-r3-impl.md'
  level: protocol
  proposal: '协议明确规定：代码已落地、只有验收被执行环境阻断，并且证据里有不依赖应用代码的独立复现时，实现轮把里程碑标为「待审」，并在证据文件首行写明阻断项'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: tooling
  symptom: '实现 #2 与实现 #3 的 ci-local 都没能跑完：Desktop 105 个中 1 个失败（electron dom harness 子进程 status=null，Electron 探针 SIGABRT），Web test 先报 EMFILE（文件描述符上限 256），实现 #3 跑了约 395 秒后出现服务启动测试失败、仍无用例汇总，人工中止退出 130；本地监听探针返回 EPERM。两轮都拿不到 Web 测试总数，也就无法与基线 301 比对；而宿主门禁在 a8116e38f4 与 df489780fe 上跑同一命令都是退出码 0'
  root_cause: '实现轮会话的沙箱不允许回环监听、也不允许启动 Electron，宿主门禁的环境允许；每个实现轮都重新复现一次阻断，并在跑不完的 Web test 上耗时'
  evidence: 'docs/bug-loop-的定位-修复-复核/evidence/M-02-r3-impl.md'
  level: rule
  proposal: '验收命令需要监听 127.0.0.1 的任务，实现轮会话以允许回环监听的权限启动，判据是实现证据里不再出现 `listen EPERM`'
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: verification
  symptom: '审计 #3 的「cargo test --workspace 补记」写「全量结果：仅 scheduler::tests::gate_queue_upgrade_collects_two_old_jobs_without_killing_them 失败，其余全过」，并据此关闭 M-03。这个用例属于 automed 的 lib 单测二进制，cargo test 默认在第一个失败的测试二进制处停下，同一 crate 的 tests/end_to_end.rs 没有运行。复盘 #2 单跑 end_to_end 里的 every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task，确定性失败（断言 protocol/v3@，实际 protocol/v4@），而审计证据里只记了一个失败。审计没有留下全量运行的日志，无法核对实际跑了哪些测试二进制'
  root_cause: '把「出现失败的那次全量运行」读成「其余测试都跑过且通过」，没有核对每个测试二进制是否都有 test result 行；时序不稳定的失败恰好挡住了后面的确定性失败'
  evidence: 'docs/bug-loop-的定位-修复-复核/evidence/M-03-r4-audit.md'
  level: rule
  proposal: '`cargo test --workspace` 有任何失败时，证据里写「其余全过」之前，必须先用 `cargo test --workspace --no-fail-fast` 重跑，并把每个测试二进制的 `test result:` 行（或其总数与失败数）写进证据'
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
