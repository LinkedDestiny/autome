# T-70 复盘教训

本任务没有记录到 Autome 指标，下面不引用任何数字指标，只引用证据文件里写明的事实。
运行概况（来自 `retro.md` 与设计状态块）：设计 1 轮、实现 6 轮、5 个里程碑全部一次审计通过、reopen 0；
唯一多出的一轮是 M-01 的实现 #2（宿主快检 Clippy 退回）。

```yaml
- id: L-01
  domain: process
  symptom: M-01 实现 #1 没跑 clippy 就标待审，宿主快检在 merge_queue.rs:82 报 clippy::nonminimal_bool，多花了实现 #2 专门修一行；实现 #1、#3、#4、#5、#6 的证据都写明“按本轮指令未运行宿主快检”
  root_cause: 设计「验证安排」与任务验收 8 要求“每个实现轮跑 cargo fmt --check 与 clippy -D warnings 并记录退出码”，而实现轮提示词写“不要自己跑项目门禁命令”，实现轮按提示词执行，两份指令冲突无人裁定；审计也没有把证据里缺这两条退出码当作验证缺口
  evidence: docs/合并队列-合并前门禁出队时才-rebase/evidence/M-01-r2-impl.md
  level: brief
  proposal: 改动了 Rust 文件的实现轮，证据文件必须写出 `cargo clippy --all-targets -- -D warnings` 的退出码；实现轮简报要明确写出这条命令不算被禁止的“项目门禁命令”
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: task, horizon: 3}
```

```yaml
- id: L-02
  domain: verification
  symptom: 审计 #3 第一次用 sed 注入负向对照，在 macOS 上语法错误、实际没注入，跑出“全绿”；实现 #6 第一次在副本上用 sed 删行，表达式没删中；两次都是事后才发现注入没生效
  root_cause: 负向对照只看测试结果，不先确认缺陷真的注入了，所以“注入失败”和“测试没拦住”看起来一样，都是全绿
  evidence: docs/合并队列-合并前门禁出队时才-rebase/evidence/M-03-r4-audit.md
  level: rule
  proposal: 每条负向对照在跑测试之前，证据里要记下注入后的 `git diff --stat`（副本则记 `diff` 行数），且不为空；为空的那次不算对照
  predicted_impact: {metric: verification_gaps, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-03
  domain: design
  symptom: 实现 #5 按设计 D7 去测 `task.list`，得到 `UnknownMethod task.list` 报错，才发现 `autome ls` 实际调用的是 `project.get`，只好在实现轮里就地改写设计 D7 和 R4
  root_cause: 设计轮写了一个当前源码里根本没有的 RPC 方法名，又没标成“新增”；评审轮和裁决轮都没有拿它去对 dispatch.rs 的方法表
  evidence: docs/合并队列-合并前门禁出队时才-rebase/evidence/M-04-r5-impl.md
  level: rule
  proposal: 设计文档里点名的每个 RPC 方法名，要么在当前 `crates/automed/src/dispatch.rs` 里能 grep 到，要么在设计里写明“新增”
  predicted_impact: {metric: impl_defects, direction: down, scope: project, horizon: 5}
```

```yaml
- id: L-04
  domain: tooling
  symptom: M-03 的 e2e 修复会话以 exit 141 退出，修复脚本没把文档写完整，表现为回归测试保护或复核字段报错、复盘重跑，调了好几次夹具才定位
  root_cause: 替身模型 CLI 的 stdout 就是事件流，脚本里直接调用的 git rebase/merge 把进度输出写进了 stdout，引发 SIGPIPE
  evidence: docs/合并队列-合并前门禁出队时才-rebase/evidence/M-03-r4-impl.md
  level: test
  proposal: 在 crates/automed/tests/end_to_end.rs 里，替身 CLI 脚本调用的每条 git 命令都要把输出重定向开，不写进 stdout
  predicted_impact: {metric: total_turns, direction: down, scope: project, horizon: 5}
```
