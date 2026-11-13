# 教训

本任务没有记录到指标，下面只引用证据文件和 git 记录里写明的事实。整轮运行：设计 1 轮，实现 11 轮，6 个里程碑全部关闭，reopen 0 次；审计 9 次，前 6 次各对应一个里程碑、均一次通过，审计 #7～#9 时已无待审里程碑。11 个实现轮里有 5 轮不推进里程碑：#4、#6 修宿主快检报的 Clippy 问题，#9 做 rebase 集成复验并修好一个在已关闭里程碑里一直红着的测试，#10 修合并前全量门禁（merge_gate，提交 fa9d68c9，退出码 1）报出的 3 个失败用例，#11 按用户追加的 rebase 指令把分支接到 main 02a05496 上，并解决与并行任务 T-54 的迁移版本冲突。

本轮（复盘 #4）保留 L-01～L-04，补 L-05。实现 #11 的证据又一次写着「未运行全量门禁、cargo fmt/clippy 或 Web lint/typecheck 宿主快检」，和 L-01 说的是同一个缺口，没有另立条目。L-05 来自实现 #11：本任务和 T-54 在各自的 worktree 里同时开发，都把账本迁移写成 v6。

```yaml
- id: L-01
  domain: tooling
  symptom: 实现 #3（M-03）和实现 #5（M-04）提交后，宿主快检分别报 clippy::cloned_ref_to_slice_refs 和 clippy::filter_next，两处都在本轮新增的测试代码里；为此多跑了实现 #4、#6 两轮修复，占 10 个实现轮中的 2 个
  root_cause: 实现轮把提示词里「不要自己跑项目门禁命令」理解成快检也不跑，只做了 rustfmt/cargo fmt，没跑 cargo clippy --all-targets -- -D warnings（M-02 证据原话是「未运行 cargo fmt --check、clippy 或 Web 快检，按本轮指令由宿主承担」；实现 #9、#10 的证据也写着「未运行全量门禁或宿主快检」），只覆盖测试目标的 lint 要等宿主快检才发现
  evidence: docs/排队任务可以调整顺序并声明依赖-task/evidence/M-04-r6-impl.md
  level: brief
  proposal: 改了 Rust 代码的实现轮，在标「待审」前原样跑一遍宿主快检命令（cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings …），把退出码写进本轮证据；没有这条记录的待审实现轮算没照做
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: design
  symptom: 实现 #7（M-05）新写的 deps_cli_queue 一开始就红：queue move 成功了，ls 顺序却没变。查下来是 project.get 一直缺设计 §6 规定的 queue_position，M-02～M-04 都已关闭，没有任何验收发现
  root_cause: 裁决 D1-N1 把 queue_position 并进了设计 §6 和 §7，可「修改落点」只写了章节、没写里程碑；采纳 D1-P01～P04 时都写到了 M-0x。结果 M-02 的用例清单里 project.get 只查 after/blocked_by，这个字段没有任何里程碑的用例来验证
  evidence: docs/排队任务可以调整顺序并声明依赖-task/evidence/M-05-r7-impl.md
  level: brief
  proposal: 裁决表里每条「采纳」或「采纳（并入设计）」的「修改落点」，至少要写出一个里程碑编号，以及该里程碑里验证这项改动的用例；只写方案章节的采纳项，裁决轮不得据此定稿
  predicted_impact: {metric: verification_gaps, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-03
  domain: verification
  symptom: 实现 #7（M-05，提交 5914f549）为补 queue_position 改了 crates/automed/src/dispatch.rs 的 project_get，但只跑了 M-05 的 CLI 验收命令；M-02 已关闭的 after_create_reads_dependencies_and_input_boundaries（比较 project.get 与 task.get 的完整任务对象）因此变红。审计 #5、#6 也都没发现，直到实现 #9 rebase 后跑 after_create_ 才暴露（5 个中 1 个失败，唯一差异是 queue_position:null），证据写明「旧分支也有同一断言」
  root_cause: M-05 的验收命令只覆盖 autome-cli（cargo test -p autome-cli …），这轮的产品改动却越出了 M-05 的范围，进了 M-02/M-03 所在的 automed dispatch.rs。M-03 的验收命令写了「after_create_ 仍然通过」，M-05 没有；实现轮和审计轮都按本里程碑的验收命令划定复验范围，没有把这个文件对应的已关闭里程碑验收命令带上
  evidence: docs/排队任务可以调整顺序并声明依赖-task/evidence/M-05-r9-impl.md
  level: brief
  proposal: 如果实现轮改了某个产品文件，而本里程碑的验收命令覆盖不到它所在的 crate 或包，标「待审」前必须跑一遍以该文件为测试位置的所有已关闭里程碑的验收命令，并把命令和退出码写进本轮证据
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-04
  domain: design
  symptom: 6 个里程碑全部关闭后，合并前全量门禁（提交 fa9d68c9）失败：Rust e2e 177 个中 1 个失败（bug_loop_task_get_reports_the_fix_round_for_bugs_only 的完整字段清单缺 after、blocked_by），Web 538 个中 2 个失败（dashboardDouble 缺 queued，导致真实 dashboard 顶层键比较失败，连带下一个用例取到空通知基线）；为此多跑了实现 #10，产品代码一行没改，只补这两份已有测试的字段契约
  root_cause: 设计 §6 给 task_json 加了 after/blocked_by、给 dashboard.get 加了 queued 组，但 §10「现有测试的必要改动」没有检索对这两个对象做整键集比较的已有测试，只列了版本钉和 write-gate 的 specific 表，结尾还断言「现有的需求与 Bug e2e、CLI 测试、调度单测一行不改」；背景一节用「现有读者都是按键名取值，新增一个键不会影响它们」论证了产品读者，没有论证测试读者。M-02、M-03 的验收命令只跑 after_create_/after_blocked_ 等子集，这两份测试要到合并前全量门禁才会运行。实现 #10 才把这两行补进 §10（git diff 583fb585 8d896cca）
  evidence: docs/排队任务可以调整顺序并声明依赖-task/evidence/M-06-r10-impl.md
  level: brief
  proposal: 设计给任何 RPC 返回对象（如 task_json、dashboard.get）新增字段时，「现有测试的必要改动」一节必须写出对该对象做整对象或整键集比较的检索命令（覆盖 Rust 测试与 Web/桌面测试替身）和全部命中项，并把命中用例放进引入该字段的那个里程碑的验收命令；没写检索命令的，评审轮按「里程碑不可执行」提出
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-05
  domain: design
  symptom: M-01 按设计「store v6」实现，并于审计 #1 关闭（3b023206，11-13 12:23）。并行任务 T-54 也把 v6 用在 sessions.limit_minutes 上（e0f6537a，12:58），并于 19:09 合进 main 02a05496。实现 #11 rebase 时 store.rs 冲突，依赖表只能顺延到 v7，设计文档和技术设计里的 v6 全部改成 v7（788adc42），已关闭的 M-01 标题、用例说明和负向对照也跟着改
  root_cause: 任务文件约束 5 写的是相对规则「SCHEMA_VERSION 加一（现为 5）」，设计 #1（17fbb986，09:23）把它定成了绝对版本号 v6，把 v6 当作本任务独占。可是 Autome 的多个任务在各自 worktree 里并行开发，下一个迁移版本号是它们共用的资源，谁先合进 main 谁拿到；设计、M-01 的验收和审计都只看本分支，没有核对 main 和其它在途任务有没有占用同一个版本
  evidence: docs/排队任务可以调整顺序并声明依赖-task/evidence/M-01-r11-impl.md
  level: rule
  proposal: 新增账本迁移的任务，每个 rebase 或合并前的集成轮都要在证据里记下 `git show main:crates/automed/src/store.rs | grep 'const SCHEMA_VERSION'` 的输出，并确认本分支的新迁移块版本号大于它；设计文档在里程碑表和验收命令里不写死迁移版本号，只写「main 当前版本 + 1」
  predicted_impact: {metric: closed_then_contradicted, direction: down, scope: task, horizon: 3}
```
