# 教训 · 在-plugins-新增-autome-插件

本任务没有记录到 Autome 指标，下文不引用指标数值；事实取自设计文档状态块（design-round 1/15、implementation-round 8/40、8 个里程碑全部 reopen 0）、`retro.md` 与证据文件。

```yaml
- id: L-01
  domain: process
  symptom: 用户在实现第 2 轮前采纳 B-1～B-3，原话是「M-04 V7 实现时同时校验键与值」「实现时在 SKILL.md 里统一写明」；实现轮把三条登记为新里程碑 M-06～M-08 排在最后，8 个实现轮里 3 个（外加 3 个审计轮）花在这三条上。M-03 写 SKILL.md 时明知第 1 步环境采集的矛盾仍保留，留给 M-08 返工；M-04 的契约负向对照本来就要求 pending 让 V7 失败，审计第 4 轮已实测拒绝，M-06 只是给同一检查再补一个 V10
  root_cause: 协议规定「纳入的 Backlog 条目要成为新的里程碑并实现」，没有区分改动是否落在某个仍开放的里程碑的交付文件或验收契约之内；实现轮按字面执行，并按「只推进最小开放里程碑」把本可顺手完成的改动推迟成独立轮次
  evidence: docs/在-plugins-新增-autome-插件/evidence/M-03-r3-impl.md
  level: protocol
  proposal: 纳入的 Backlog 条目若点名了某个仍开放的里程碑，或改动的文件属于某个开放里程碑的交付物，就并入该里程碑的验收契约，不新建里程碑；只有在所有开放里程碑都覆盖不到时才新建
  predicted_impact: {metric: impl_rounds_used, direction: down, scope: project, horizon: 3}

- id: L-02
  domain: tooling
  symptom: 实现第 1～8 轮每轮都在沙箱内跑 sh scripts/ci-local.sh（第 1 轮 144 秒、第 3 轮 382 秒），每轮都退出 1，都是同样 11 个 Web 用例因 listen EPERM 127.0.0.1 失败，结果逐轮不变；同期 Autome 在提交 fc752e0 与 8c9c0b4 上跑的宿主门禁都退出 0，并写进了审计简报
  root_cause: 沙箱禁止本机监听，所以实现轮跑全仓门禁永远不可能变绿，只能重复记录已知的环境失败；真正有效的全仓门禁由 Autome 在提交后在宿主上运行
  evidence: docs/在-plugins-新增-autome-插件/evidence/M-03-r3-impl.md
  level: brief
  proposal: 宿主门禁已经覆盖 scripts/ci-local.sh 时，实现轮简报写明「全仓门禁以提交后的宿主门禁为准」，实现轮证据里不再出现沙箱内的 ci-local.sh 全量运行，只跑本里程碑验收命令与它所在的测试组
  predicted_impact: {metric: total_tokens, direction: down, scope: task, horizon: 3}

- id: L-03
  domain: design
  symptom: 设计初稿让 Agent 用固定定界符 heredoc 把用户整理的任务卡正文拼进 shell 命令，先送检再创建；评审第 1 轮用 /bin/sh 探针证明，正文里独占一行的定界符会提前截断输入，后面的行会在确认之前被当成命令执行，校验脚本根本收不到那一行（P01，阻塞，违反 C2/A1）
  root_cause: 设计把「校验脚本拒绝定界符」当成防线，却忽略了 shell 在启动接收端之前就已解析 heredoc；用户或对话产出的文本被当成命令文本而不是数据传输
  evidence: docs/在-plugins-新增-autome-插件/在-plugins-新增-autome-插件-review.md
  level: rule
  proposal: 设计里凡是 Agent 把用户或对话产出的文本交给命令行，一律先用宿主文件写入能力写成文件，再用 < 文件 重定向读入；设计正文与技能文件中不得出现 heredoc（<<）承载这类文本
  predicted_impact: {metric: design_rounds_used, direction: down, scope: project, horizon: 3}

- id: L-04
  domain: design
  symptom: 设计「背景与现状」断言不带 --draft 创建时返回 started true，并据此只写了待开始、已开始两种回显；评审第 1 轮查到 crates/automed/src/dispatch.rs 1497-1504 只在调度实际启动时置 started=true，已有测试 create_without_the_new_params_is_unchanged 断言槽位占满时 state=queued、started=false、queue_position=1（P02，阻塞）
  root_cause: 对现有接口返回值的现状描述没有对照源码与已有测试，把「进入调度」写成了「已经开始执行」；规划轮做过事实核对，但没有覆盖这一条字段语义
  evidence: docs/在-plugins-新增-autome-插件/在-plugins-新增-autome-插件-adjudication.md
  level: rule
  proposal: 设计「背景与现状」里每一条关于现有命令或接口返回字段取值的断言，都要附上源码 文件:行号 或已有测试名；附不出的断言不得作为分支设计的依据
  predicted_impact: {metric: design_rounds_used, direction: down, scope: project, horizon: 3}
```
