# 内核补齐批次（T-95）：教训

复盘 #1，2026-11-14。本任务没有记录到 Autome 指标，下列条目只引用证据文件里的事实；L-01 的失败已在复盘轮于 HEAD `6971f9bc`
复跑确认（`cargo test --offline -p automed --lib protocol::`：64 过、8 败，均为「D-1：eval 目录 `evals/audit-checks-operator-notes/` 不存在」，
输出在不入库的 `.autome/output/retro-protocol-lib.txt`）。

```yaml
- id: L-01
  domain: verification
  symptom: 审计 #9 让 M-09 通过；后来实现 #17（M-16）跑 `cargo test -p automed --lib protocol::` 得到 8 个失败，都是「D-1：eval 目录 `evals/audit-checks-operator-notes/` 不存在」，复盘轮在 HEAD 复跑仍是 64 过、8 败
  root_cause: M-09 的验收命令只有 `protocol eval protocol/`，读的是磁盘目录，目录确实在；而编译进二进制的 seed 要在 `crates/automed/src/protocol/evals.rs` 的 include_str! 清单里逐个登记 eval 文件，M-04 登记了 A-1，M-09 没登记 D-1；唯一能拦住它的 `--lib protocol::` 只写进了 M-04 的契约，没写进 M-09 的
  evidence: docs/内核补齐批次-2026-11-13-4/evidence/M-16-r17-impl.md
  level: rule
  proposal: 改动 `protocol/CHANGELOG.md` 或 `protocol/evals/` 下文件的里程碑，验收命令必须包含 `cargo test -p automed --lib protocol::`
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: process
  symptom: 实现 #17 和审计 #16 把这 8 个失败记成「既有阻塞」，M-16 照常通过，状态块写成 `已完成`，失败的用例原样留在分支上，交给合并前门禁处理
  root_cause: 「既有」只核对了「把 M-16 的产品改动还原后同样失败」，没有核对失败是任务基线之前就有的，还是本任务提交引入的（D-1 来自 M-09 的提交 7ae0f52b）；本任务引入的失败没有退回对应的里程碑
  evidence: docs/内核补齐批次-2026-11-13-4/内核补齐批次-2026-11-13-4-audit.md
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
