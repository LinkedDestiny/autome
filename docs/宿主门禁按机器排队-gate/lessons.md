# 教训 · 宿主门禁按机器排队（T-47）

复盘轮 #1 写。依据：`retro.md`、`evidence/` 下 12 份证据、审计文件、裁决文件、设计文档状态块与 `## Backlog`，另外对照了其它任务的 `lessons.md`，同义的条目逐字沿用它们的 proposal，方便跨任务计数。

**运行概况（取自状态块与证据）。** 设计 2 轮定稿（评审 #1 提出 5 项阻塞，评审 #2 提出 2 项，都被采纳）。实现 6 轮，M-01..M-06 每个都是一次审计就关闭，reopen 0，实现缺陷 0。验证缺口 1 个：审计 #1 发现 clippy `items_after_test_module`，当场修掉（L-01）。H-01 这条人工验收还没确认。

**指标与实际不符的地方：**
- 指标写「实现轮 2/30」，状态块是 `implementation-round: 6/30`，evidence 下也有 r1..r6 六份实现证据（L-05）。
- 指标写「协议失败 2」，本任务所有文件里都找不到对应记录：状态块从没出现过 `协议失败`，`retro.md`、证据和 git log 里也都没有。写不出证据路径，所以不立条目。
- `retro.md` 缺评审 #1、裁决 #1、裁决 #2 三行，也缺终止总结。这一条同样没有指标能衡量，不立条目。

跨任务复用：L-03 的 proposal 与 `门禁连续因同一原因失败时熔断-停下等人` 的 L-06、`用户按合并时如果默认分支已经前进-任务自动` 的 L-04 相同；L-05 的 proposal 与 `在-apps-web-建-react` 的 L-05、`服务端每次-tick-后用-events` 的 L-07 相同。两条 proposal 都一字未改。L-01、L-02、L-04 是本任务新出现的。

```yaml
- id: L-01
  domain: verification
  symptom: '审计 #1 跑 `cargo clippy --workspace --all-targets -- -D warnings`，报 items_after_test_module：config_io.rs 里的 parse_gate_concurrency 放在了测试模块之后。审计当场修掉，记了 1 个验证缺口。实现 #1 跑的是 `cargo clippy -p autome-domain -p automed --lib`，没有 --all-targets，所以没发现。M-05 也是同类情况：实现 #5 没跑 clippy，新增 e2e 里 Ctx::new 的两处 needless_borrows_for_generic_args，到 M-06 才被实现 #6 第一次跑 clippy 时发现（m06-clippy.txt）'
  root_cause: '设计中 M-01..M-05 的验收命令都没有 clippy（设计文档第 433、450、478、490、506 行），只有最后的 M-06（第 517 行）带了 `cargo clippy --all-targets -- -D warnings`。实现轮按验收命令自选 clippy 的范围，甚至可以不跑；lint 欠账要么被审计顺手撞上，要么拖到收尾里程碑'
  evidence: 'docs/宿主门禁按机器排队-gate/evidence/M-01-r1-audit.md'
  level: rule
  proposal: '凡改动 Rust 代码的里程碑，其验收契约的验收命令必须包含 `cargo clippy --all-targets -- -D warnings`，不得只放在最后的全量里程碑'
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-02
  domain: design
  symptom: '两轮评审一共 7 项阻塞，其中 5 项属于同一类：名额被持有之后，没有人能收回它，或者它被漏计。D1-P01：head 变化后旧作业永久占位。D1-P02：暂停期间释放的准备作业，恢复后免准入。D1-P04：故障队首、暂停的挂死作业让等待无限期。D2-P01：Active 任务的业务路径不可达（head 读不到、[setup] 被移除）后，作业无人超时、无人收取。D2-P02：读不到目录时按 0 计数。每一项都由裁决轮补一个专门的机制（旧 head 清扫、接续资格、空闲任务回收、规则 (b) 的期限、三值探测），用掉了 2 个设计轮'
  root_cause: '设计初稿默认「Active 任务必然由业务路径 poll，非 Active 才需要回收」，没有逐一列出名额持有者可能处于的每种状态，以及每种状态下由谁释放名额。评审只能一个反例一个反例地把这些状态找出来'
  evidence: 'docs/宿主门禁按机器排队-gate/宿主门禁按机器排队-gate-adjudication.md'
  level: rule
  proposal: '设计引入名额、锁或互斥计数时，必须有一张表逐行列出持有者的每种状态（至少含 Active、暂停、停止或取消、检出 head 变化、相关配置被移除、作业路径不可读、进程已退出未收取），并为每行写出不依赖该任务业务路径的释放方'
  predicted_impact: {metric: design_rounds_used, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-03
  domain: process
  symptom: '两条与本任务无关的旧 e2e 用例，在默认并发跑整套时各失败过一次。第一条是审计 #5 遇到的 a_session_that_finished_during_a_restart_is_consumed_on_recovery：等 .exit 标记的固定 10 秒超时，在 174% CPU 下超时。审计判为负载抖动，只写进了证据，没进设计文档 `## Backlog`。第二条是审计 #6 遇到的 stopping_a_task_kills_the_cli_the_wrapper_started：end_to_end.rs:2978 处 ParseIntError: Empty，原因是读到了刚创建、还没写入内容的 cli.pid。审计把它写进了 Backlog，但用的编号 B-07 已经被 D1-N01 的 T3 对照建议占用，现在 `## Backlog` 里有两条 B-07。两次审计都靠单跑 3 次加整套重跑来判定，又多花了一轮全量 e2e'
  root_cause: '无关失败的诊断只写在当轮的证据里，后续轮次读的是 Backlog，看不到它。补进 Backlog 时，也没有核对已有的 ID'
  evidence: 'docs/宿主门禁按机器排队-gate/evidence/M-05-r5-audit.md'
  level: rule
  proposal: '实现轮或审计轮遇到与本任务改动无关、且按范围约束不修的测试或门禁失败时，必须在设计文档 `## Backlog` 写一条，含失败用例全名、断言原文与机制'
  predicted_impact: {metric: total_turns, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: '设计写「autome-domain 共 394 个用例全部通过（基线 385 + 新增 9）」。385 是 `--list` 的总数，里面含 1 个 ignored 的集成测试，实际最多只能 393 个通过。实现 #1 证据写清了「385 = 384 单测 + 1 原有忽略」；审计 #1 却把差 1 判成「契约基线笔误」，说实测基线是 384，审计 #2 沿用了「基线笔误」这个说法。直到审计 #4 才写明「契约写的基线 394 是含 ignored 的口径」。连续三轮审计都要先花时间对数，才能下结论'
  root_cause: '验收契约把 `--list` 总数（含 ignored）和「全部通过」写在同一句里，没写数的是哪一种口径。实现、审计各按自己的口径对数，对不上时容易误判成笔误或丢了用例'
  evidence: 'docs/宿主门禁按机器排队-gate/evidence/M-04-r4-audit.md'
  level: rule
  proposal: '验收契约里的用例数必须写明口径：写 `--list` 总数时同时写出其中 ignored 的个数，并把「全部通过」改写成「通过数 = 总数 − ignored 数」'
  predicted_impact: {metric: total_turns, direction: down, scope: project, horizon: 3}
```

```yaml
- id: L-05
  domain: tooling
  symptom: '复盘 #1 收到的指标写「实现轮 2/30」。启动复盘时，设计文档状态块是 `implementation-round: 6/30`，`retro.md` 有实现 #1..#6 六行，evidence 下也有 M-01-r1-impl.md..M-06-r6-impl.md 六份实现证据。设计轮那一项（2/15）与状态块一致'
  root_cause: '指标里的 impl_rounds_used 不取自状态块 implementation-round 的分子，或者在实现 #2 之后就没再刷新。具体是哪个环节，本轮只读文档，没有读 automed 源码确认'
  evidence: 'docs/宿主门禁按机器排队-gate/宿主门禁按机器排队-gate.md'
  level: test
  proposal: 'Autome 增加一条测试：任务结束后，指标中的 design_rounds_used 与 impl_rounds_used 分别等于任务分支上设计文档状态块 `design-round`、`implementation-round` 的分子'
  predicted_impact: {metric: impl_rounds_used, direction: up, scope: task, horizon: 1}
```
