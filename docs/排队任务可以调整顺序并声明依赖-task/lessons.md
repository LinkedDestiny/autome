# 教训

本任务没有记录到指标，下面只引用证据文件和 git 记录里写明的事实。整轮运行：设计 1 轮，实现 9 轮，6 个里程碑全部关闭，reopen 0 次；审计 7 次，前 6 次各对应一个里程碑、均一次通过，审计 #7 时已无待审里程碑。9 个实现轮里有 3 轮不推进里程碑：#4、#6 修宿主快检报的 Clippy 问题，#9 做 rebase 集成复验，并修好了一个在已关闭里程碑里一直红着的测试。

```yaml
- id: L-01
  domain: tooling
  symptom: 实现 #3（M-03）和实现 #5（M-04）提交后，宿主快检分别报 clippy::cloned_ref_to_slice_refs 和 clippy::filter_next，两处都在本轮新增的测试代码里；为此多跑了实现 #4、#6 两轮修复，占 9 个实现轮中的 2 个
  root_cause: 实现轮把提示词里「不要自己跑项目门禁命令」理解成快检也不跑，只做了 rustfmt/cargo fmt，没跑 cargo clippy --all-targets -- -D warnings（M-02 证据原话是「未运行 cargo fmt --check、clippy 或 Web 快检，按本轮指令由宿主承担」），只覆盖测试目标的 lint 要等宿主快检才发现
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
