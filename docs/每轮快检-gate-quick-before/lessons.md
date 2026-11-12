# 教训 · 每轮快检（`[gate] quick`，T-67）

复盘轮 #2 覆盖写。依据：任务文件、`retro.md`、`evidence/` 下 12 份证据、评审与裁决记录、审计记录、任务分支 `git log` 与
reflog，以及 `.autome/output/r6-host-failures.txt`（实现 #6 摘出的合并前门禁日志摘要，只读）。Autome 本任务没有记录到指标，
下文不引用任何指标数值；文中的数字都取自证据文件、门禁日志或 `git` 的实际输出。

可核对的运行事实：

- 设计 1 轮（评审 #1 提出 2 项阻塞意见 R1-01、R1-02，裁决 #1 全部采纳并定稿）。实现 6 轮、审计 6 轮；M-01..M-05 各一次审计
  通过，没有 reopen，所有审计记录都没有实现缺陷或验证缺口。
- 复盘 #1 之后，分支 rebase 到 main `bf399db`，合并前完整门禁在 `d4b93c2` 上失败，两步红：
  - Clippy：`scheduler::Route::Rerouted(ResolvedConfig)` 触发 `large_enum_variant`（至少 224 字节）。`Route` 在分支基线
    `042fd06` 上就有；5 个实现轮在 rebase 前都记了 clippy 退出码 0。main 在 `042fd06..bf399db` 之间给 `ResolvedConfig` 加了
    `merge_after`，本分支加了 `gate_quick`，两者叠加才越过阈值——这是 rebase 产生的语义冲突，会话在 rebase 前无从发现，
    所以不单列为教训。
  - Test：automed lib 793 个用例 1 个失败（`protocol::tests::the_version_in_force_is_the_newest_changelog_heading_over_the_seed`，
    预期 v4、实际 v5），耗时 111.56 秒；cargo 在这个二进制失败后停下，其后的 Rust 集成测试没有执行。
- 实现 #6 把 `Route::Rerouted` 改为 `Box<ResolvedConfig>`，并把两处 `protocol/v4` 断言改为 v5（第二处在 e2e，靠 grep 找到）；
  审计 #6 通过。复盘 #2 时合并前门禁还没有重跑，结果未知。
- 人工验收 H-01、H-02 未确认。

L-01..L-04 沿用复盘 #1，L-04 补了实现 #6 的同一现象。L-05、L-06 是合并前门禁失败带来的新教训。

```yaml
- id: L-01
  domain: design
  symptom: '评审 #1 R1-02（裁决 D1-P02 采纳）：设计初稿让作业按启动时的 kind 收结果，却让 Bug 复核的宿主门禁说明 {fix_gate_note} 只出现在按当前配置选出的 before_merge 分段里，且非 quick 结果一律回落成固定句「修复门禁只跑了复现命令」。评审给出三条可由设计推出的错误路径：快检在途切 each_round，复核丢掉说明；each_round 的完整 FixGate 在途切到 before_merge + quick，复核被告知「只跑了复现命令」；需求快检在途切 each_round，审计简报仍写「全量门禁只在合并前运行」'
  root_cause: '设计把「这次实际跑了什么」（应取自启动时落盘的 kind / command / exit_code）与「当前配置规定以后跑什么」（每个 tick 重新解析）放进了同一个由当前配置选择的模板分段和固定句子里。这与 T-60 L-03 同源：那次是 gate.result.command 在收结果的 tick 取当时配置，这次是陈述文字的有无与措辞由当前配置决定'
  evidence: docs/每轮快检-gate-quick-before/每轮快检-gate-quick-before-review.md
  level: rule
  proposal: '设计里凡向人或会话陈述「实际执行了什么」的字段或文字（账本字段、简报段落、prompt 分段与补充句），必须写明它取自作业启动时的记录还是当前配置；取自当前配置的只能陈述以后的策略，不得陈述已发生的运行'
  predicted_impact: {metric: design_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: tooling
  symptom: '审计 #3 第一次负向对照用 BSD sed 注入「禁用 FixGate 快检分支」，改动实际没落到文件上，4 个用例全过；审计发现后改用 python 重做，lib 3 个、e2e 2 个才变红。这是同类空转第三次出现（T-61 审计 #5、文档补齐-bug-loop 审计 #1 都是 BSD sed 静默不改）'
  root_cause: 'macOS 的 BSD sed 与 GNU sed 的 -i 及地址语法不同，不匹配时静默不改、退出 0；负向对照只看「测试红没红」，不先确认注入已落盘。本次靠审计对「全过」起疑才没记成「验收命令拦不住」'
  evidence: docs/每轮快检-gate-quick-before/evidence/M-03-r3-audit.md
  level: rule
  proposal: '每条负向对照在跑验收命令之前，证据里必须记录注入后 git diff --stat 的输出，且被注入的文件出现在输出中；没有这条记录的对照不得写成「拦得住」或「拦不住」'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: tooling
  symptom: '实现 #5 实测任务卡给本仓库拟的快检命令时，第一次在 npm run lint --prefix apps/web 处退出 127（eslint not found），手动 npm ci --prefix apps/web 之后才跑通（整条 6.00 秒）。人工验收 H-01 只要求在主检出 [gate] 加上这条 quick'
  root_cause: '新建的任务 worktree 没有 apps/web/node_modules。完整门禁 scripts/ci-local.sh 在 node_modules 不存在时自己跑 npm ci，所以今天看不出问题；快检命令没有这一步。本 worktree 与主检出的 .autome/config.toml 都没有 [setup]（只读查看），宿主在快检前不会装依赖，于是照 H-01 原样启用后，每个新任务第一个实现轮的快检都会以 127 失败并退回实现轮'
  evidence: docs/每轮快检-gate-quick-before/evidence/M-05-r5-impl.md
  level: rule
  proposal: '主检出 [gate] 启用含 npm 步骤的 quick 时，同一次改动必须让新建 worktree 在快检前具备 apps/web/node_modules（加 [setup] command = "npm ci --prefix apps/web"，或快检命令自带 node_modules 缺失时的安装）；H-01 的确认以一个新建任务第一个实现轮的快检退出码为 0 为准'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: verification
  symptom: '实现 #2..#6 按协议报「用例总数与上一轮基线的差」，五轮都是跨里程碑或跨性质比较，五轮都得花一段解释差值不是丢用例：#2 写 -2、#3 写 -15、#4 写 +44、#5 写 -42，#6（门禁修复轮）写 +31 并另起一句「本轮没有增删测试，同范围净变化 0」。真正有用的是各轮另外自发实跑的同一验收命令改前基线（#5：CLI 9 → 11，桌面 0 → 3；#4：43 → 56）'
  root_cause: 'loop-protocol「验收结果报什么」要求与「上一轮基线」比，而上一轮通常是另一个里程碑、另一组验收命令；两组命令的用例数之差既不能发现整层被跳过，也不能证明新增了多少，只有同一命令改前改后的差才能'
  evidence: docs/每轮快检-gate-quick-before/evidence/M-04-r6-impl.md
  level: protocol
  proposal: '实现轮报验收用例总数时，基线是同一条验收命令在本轮改动前实跑的结果（证据里写出该命令与改前、改后两个数）；与上一里程碑的用例总数比较不作要求'
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: design
  symptom: 'M-04（实现 #4）按设计 §9 在 protocol/CHANGELOG.md 发布 v5，分支基线 042fd06 上已有两处测试把版本钉成字面量 protocol/v4（protocol::tests 一处、end_to_end 一处）。设计 §13 写「其余现有单测、金样、e2e 一行不改」，M-04 验收命令只筛 gate_quick_prompt / gate_quick_brief / before_merge_prompt / brief::tests::before_merge / prompt_rendering / protocol eval，两处都不在内；before_merge 的审计 prompt 又明说全量回归由合并前门禁承担。于是这个确定性失败从实现 #4 一直活到合并前门禁，多花实现 #6、审计 #6、复盘 #2 和一次完整门禁'
  root_cause: '设计列「必须改的现有测试」时只按「结构体加字段导致编译不过」去推，没有对本任务要改掉的旧字面量（protocol/v4）做全仓搜索；在 before_merge 下，验收筛选之外的测试没有任何一轮会跑，所以设计漏列等于留到合并前才暴露。实现 #6 用 git grep protocol/v4 一次就找全了两处'
  evidence: docs/每轮快检-gate-quick-before/evidence/M-04-r6-impl.md
  level: rule
  proposal: '里程碑改动了测试里按字面量写死的值（协议版本标签、常量、文件名）时，设计的「须改的现有测试」清单必须附上对旧字面量的仓库搜索命令及全部命中，且该里程碑的验收命令包含这些命中所在的测试或对旧字面量搜索为空的检查'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-06
  domain: verification
  symptom: '合并前门禁的 Test 步在 automed lib 的 1 个失败后停下（门禁日志：792 passed; 1 failed，其后 Rust 集成测试没有执行）。实现 #6 自己写明「后续集成测试没有执行，不能把它们视为通过」，但修复后只跑了 lib 筛选 12 个、end_to_end 2 个、prompt_rendering 31 个，「未运行 sh scripts/ci-local.sh 或全量 Rust 测试」；审计 #6 同样只跑了 protocol::tests 与 1 个 e2e。被门禁中断的那部分测试在复盘 #2 时仍没有任何一轮完整跑过'
  root_cause: 'cargo test 默认在第一个失败的测试二进制后停止，门禁日志因而只暴露第一处失败，看不到被跳过的二进制里还有没有别的失败；修复轮按「修好日志里看到的那几处」验收，把未执行等同于未失败。下一次合并前门禁若在被跳过的部分再红，又是一整圈实现、审计、复盘与完整门禁'
  evidence: docs/每轮快检-gate-quick-before/evidence/M-04-r6-impl.md
  level: rule
  proposal: '修复合并前门禁失败的实现轮，证据里必须列出门禁日志中因前序失败而未执行的每个测试目标，并给出它们在修复后完整运行（可用 cargo test --no-fail-fast 或逐个目标）的退出码；会话里跑不通的逐项写明原因'
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```
