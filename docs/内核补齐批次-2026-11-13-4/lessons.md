# 内核补齐批次（T-95）：教训

复盘 #2，2026-11-14，覆盖复盘 #1 的版本。本任务没有记录到 Autome 指标，下列条目只引用证据文件里的事实。

复盘 #1 之后又跑了实现 #18（rebase 到 main `fcabc5e1`）和审计 #17（集成复验，通过）。本轮在 rebase 后的 HEAD `e8d7a84c`
复跑 `cargo test --offline -p automed --lib protocol::`：仍是 64 过、8 败，全部是「D-1：eval 目录 `evals/audit-checks-operator-notes/` 不存在」，
`crates/automed/src/protocol/evals.rs` 里 `audit-checks-operator-notes` 的登记数仍为 0（输出在不入库的 `.autome/output/retro2-protocol-lib.txt`）。
集成两轮没有碰这个失败，所以 L-01、L-03 原样保留，L-02 补上集成轮的事实，新增 L-04。

```yaml
- id: L-01
  domain: verification
  symptom: 审计 #9 让 M-09 通过；后来实现 #17（M-16）跑 `cargo test -p automed --lib protocol::` 得到 8 个失败，都是「D-1：eval 目录 `evals/audit-checks-operator-notes/` 不存在」，复盘 #1 和复盘 #2（rebase 后的 HEAD）复跑仍是 64 过、8 败
  root_cause: M-09 的验收命令只有 `protocol eval protocol/`，读的是磁盘目录，目录确实在；而编译进二进制的 seed 要在 `crates/automed/src/protocol/evals.rs` 的 include_str! 清单里逐个登记 eval 文件，M-04 登记了 A-1，M-09 没登记 D-1；唯一能拦住它的 `--lib protocol::` 只写进了 M-04 的契约，没写进 M-09 的
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-16-r17-impl.md
  level: rule
  proposal: 改动 `protocol/CHANGELOG.md` 或 `protocol/evals/` 下文件的里程碑，验收命令必须包含 `cargo test -p automed --lib protocol::`
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: process
  symptom: 实现 #17 和审计 #16 把这 8 个失败记成「既有阻塞」，M-16 照常通过；rebase 之后的实现 #18 和审计 #17 又一次写「既有 D-1 eval 缺失仍按原审计记录处理」，集成复验通过，失败的用例原样留在分支上，交给合并前门禁
  root_cause: 「既有」只核对了「把 M-16 的产品改动还原后同样失败」，没有核对失败是任务基线之前就有的，还是本任务提交引入的（D-1 来自 M-09 的提交 7ae0f52b，rebase 后为 `main..HEAD` 上的 7b3dfed1）；本任务引入的失败没有退回对应的里程碑，后续每一轮都沿用前一轮的「既有」标签
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-16-r18-audit.md
  level: protocol
  proposal: 证据把某个测试失败标为「既有」时，必须写出引入它的提交，并说明该提交早于任务基线；引入提交在本任务分支上的，必须把对应里程碑改回 `开放` 并加 reopen
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: verification
  symptom: 审计 #4 和审计 #13 各发现验收契约里有一条负向对照照原文注入后不变红：M-04 把新键写进 `bug-status-block` 契约区，`protocol::` 仍然 71 过；M-13「去掉 `-M`」时，因为 git 默认开启改名检测，用例仍绿
  root_cause: 实现轮都是换了注入方式或运行命令才看到红（M-04 改为对 `.autome/output/m04-candidate` 跑静态门，M-13 改用 `--no-renames`），证据里写的是「全部红」或只写了实际做法，没有说契约原文那条不会红，契约的缺口拖到审计才暴露
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-13-r14-audit.md
  level: brief
  proposal: 实现轮执行的负向对照，只要注入内容或运行命令与验收契约原文不同，证据就必须把契约原文和实际做法并排写出，并给出按原文注入后验收命令的实际结果
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: process
  symptom: 复盘 #1 在 retro.md 写明「HEAD 上 automed --lib protocol:: 8 败（D-1 eval 未进 seed，M-09 引入），合并前门禁会红」，紧接着的实现 #18 简报和审计 #17 简报里都没有这条（两份简报里 grep「复盘」「教训」「D-1」「protocol::」均无命中），两轮都只跑了 rebase 范围的用例，没有跑 `--lib protocol::`，失败带进了待合并的分支
  root_cause: 复盘轮发现的「HEAD 上有已知失败」只落在 retro.md 和 lessons.md，这两份文件不进实现轮、审计轮简报；所有里程碑已完成时简报的「本里程碑」「上一轮对手轮的结论」两节都是空的，没有别的通道把它送到下一轮
  evidence: docs/内核补齐批次-2026-11-13-4/brief/impl-18.md
  level: brief
  proposal: retro.md 里记了 HEAD 上未解决的测试失败时，其后第一份实现轮简报必须原文列出这条失败和复现命令，该实现轮证据必须写出这条命令在本轮 HEAD 上的结果
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
