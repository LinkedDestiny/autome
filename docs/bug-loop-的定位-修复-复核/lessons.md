# 教训 · Bug Loop 的定位 / 修复 / 复核 prompt 列出任务附件与 `--doc` 参考（T-41）

复盘轮 #1 覆盖写。依据：任务文件、`retro.md`、`evidence/` 下 5 份证据、评审 / 裁决 / 审计记录、`brief/impl-*.md`
与 `brief/audit-*.md`、任务分支 `git log 83c80e9..HEAD`。本任务没有 Autome 指标，下文不引用任何指标数值。

设计 1 轮定稿（评审 R1-01 范围越界，裁决 #1 采纳，未多开设计轮）；M-01、M-02 都是一次审计关闭，reopen 0，审计
无实现缺陷、无验证缺口。多出来的轮次只有一处：实现 #1 把 M-01 标待审后，宿主门禁在 `3e839ce` 上报 Test 失败，
实现 #2 只改了 `end_to_end.rs` 里一行版本断言（L-01、L-02）。另外两条是执行环境问题，与之前任务的同类教训
**沿用原 proposal 原文**，以便跨任务计数（L-03、L-04）。

```yaml
- id: L-01
  domain: verification
  symptom: '实现 #1（3e839ce）的 M-01 验收只跑 launcher / prompt_rendering / protocol:: 与静态门，证据写「M-02 的 e2e、workspace 全量、ci-local、插件 eval 本轮未跑，留给下一里程碑」，然后标待审；宿主门禁在 3e839ce416 上报 Test 失败（end_to_end 的 every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task 仍断言 protocol/v2@，实际 protocol/v3@），于是派出实现 #2，它只把 end_to_end.rs 一行前缀断言改成 protocol/v3@（a8116e3：end_to_end.rs 2 行改动）'
  root_cause: 'M-01 改了协议版本（全仓库可见的产品行为），但验收契约只覆盖 launcher、金样与 protocol 模块；cargo test --workspace 只排在 M-02「全量门禁」里程碑，M-01 打破 end_to_end 时实现轮自己的验收看不到'
  evidence: 'docs/bug-loop-的定位-修复-复核/evidence/M-01-r2-impl.md'
  level: rule
  proposal: '凡改动产品代码的里程碑，其验收契约的验收命令必须包含 `cargo test --workspace`，不得把全量运行只放在单独的「全量门禁」里程碑'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: design
  symptom: '定稿设计（4ec408f）「背景与现状」第 6 条只列出 protocol/mod.rs:401 的 protocol/v2 断言要改，并判 version_page.rs、autome-domain/src/changelog.rs 的 protocol/v2 为样例不用改；漏掉两处：eval.rs:818/932 的测试夹具固定追加 `## protocol/v3`，与真实新版本重名（实现 #1 首跑 protocol:: 60 通过、1 失败，当轮补进设计清单）；end_to_end.rs:1701 `starts_with("protocol/v2@")`（实现 #1 没跑到，宿主门禁报红，多花实现 #2）。两处都能在基线上用 `git grep -n "protocol/v[23]" 83c80e9 -- crates` 一次列出'
  root_cause: '设计只检索了旧版本号的部分出现位置，没有对 crates/ 全量检索旧版本号与新版本号两个字面量，并对每一条命中给出处理结论'
  evidence: 'docs/bug-loop-的定位-修复-复核/evidence/M-01-r1-impl.md'
  level: rule
  proposal: '设计把协议版本从 vN 升到 vN+1 时，必须写出在基线上对 `crates/` 执行 `git grep -n "protocol/vN"` 与 `git grep -n "protocol/vN+1"` 的全部命中，并对每一条命中写明「改」或「不改的理由」'
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
