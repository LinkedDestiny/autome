# 教训

本任务没有记录到指标，下面只引用证据文件里写明的事实。整轮运行：设计 1 轮，实现 8 轮，6 个里程碑全部关闭，reopen 0 次，审计 6 次全部一次通过。实现 8 轮中有 2 轮（#4、#6）是宿主快检报 Clippy 后的修复轮，不推进里程碑。

```yaml
- id: L-01
  domain: tooling
  symptom: 实现 #3（M-03）和实现 #5（M-04）提交后，宿主快检分别报 clippy::cloned_ref_to_slice_refs 和 clippy::filter_next，两处都在本轮新增的测试代码里；为此多跑了实现 #4、#6 两轮修复，占 8 个实现轮中的 2 个
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
