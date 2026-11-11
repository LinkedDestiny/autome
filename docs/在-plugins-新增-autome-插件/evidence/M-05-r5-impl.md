# M-05 实现 #5：eval 用例

本轮只推进 M-05，未修改 M-06–M-08。上轮审计 M-04 通过，无退回缺陷，因此没有要求逐字搬入的审计红绿回归。

## 实现与验收

新增三个 case.yaml、三个 grader、evals/README.md；新增 tests/evals.sh，接入 run.sh evals 与 all。沿用 loop 的 YAML 字段和 grader 相对路径，不使用 scaffold_script，不开放 Bash。

- `claude plugin eval --help`：退出 0，核对 case.yaml + graders 格式、runs、max_turns、timeout_seconds、ablation、max-cost-usd 与 gated tools 选项；仅查看帮助，没有运行收费评估。
- `sh plugins/autome/tests/run.sh evals`：退出 0，5/5，通过 E1–E5，无失败或跳过。
- `sh plugins/autome/tests/run.sh all`：退出 0，59/59，无失败或跳过；上一轮审计基线与本轮改动前实跑均为 54/54，增加 5 个，全部来自新增 E1–E5，原五组没有删减。设计的最终总数 59 已实现。
- 开发中 E1 首跑 4/5：错误地要求 YAML 的 graders 容器行同一行有值；按既有多行序列格式修正为检查容器、非空引用及文件存在，修后 5/5。
- `sh .autome/output/impl5/negative.sh`：五种临时副本变体全部被拒绝（5/5）；分别删阳性对照、删运行命令成本上限、增加 scaffold_script、清空 description、删除被引用 grader。原件未修改，原件全量仍 59/59。这 5 次缺陷注入不计入项目用例总数。
- `git diff --check`：退出 0。

原始输出：`.autome/output/impl5/` 下的 eval-help.txt、baseline.txt、evals.txt、all.txt、negative.txt、negative-*.txt、gate.txt。临时验证脚本同目录，不放入任务文档。

## 标待审之前的自审清单

1. **情形表逐行**：逐行核对设计三场景。
   - ask-before-inventing：prompt 只给背景、行为、排除范围与参考，不给验收标准；grader 要求先追问验收标准、禁止自编整卡和任何命令尝试，附自编 200ms/刷新保持条目的 0 分对照及追问的 1 分对照。
   - bug-missing-repro：prompt 仅现象和报错；grader 要求一次追问复现步骤、期望行为、环境，不编步骤；提供“点击新增”的 0 分对照与一次追问的 1 分对照。
   - kind-ambiguous：prompt 使用设计原句，同时包含 CSV 新增和 Excel 乱码信号；grader 要求先问类型、不默认、不提前追问模板内容，直接按 feature 或 bug 出卡均判 0。
   三行文件与判据人工逐项核对完成，E1–E3 结构检查通过；未将静态核对声称为 LLM 实际判分。
2. **失败分支**：本里程碑没有授权、读盘、服务连接等运行分支。结构错误的五种分支已用上述负向变体执行并拒绝；grader 明确工具被拒绝也不算通过，缺信息必须停止追问。真实 Agent 的行为失败与判分仍由 H-05 验收，未运行。
3. **全部迁移**：不适用。本里程碑是三个单次讨论输入及判分资料，不实现状态机或通道；三场景的预期均为追问后等待，不进入创建。创建与回显状态机属于已验收 M-03。
4. **域边界**：检查配置空 description、缺失引用 grader，均被 E1 拒绝；当前每场景恰一个 grader 的单元素输入通过 E1，每个预算/回合/超时值为正整数。日期、字节上限等运行输入域不属于本里程碑。中文提示、模板小节及阳性对照标题已逐项核对；真实收费 eval 未运行。

## 限制

H-05 真实 eval 及 grader 对阳性对照的实际 LLM 判分未运行；按 C8 不进每轮门禁，README 保留带 2.00 美元上限的指定运行命令，不把结构检查代替行为验收。

## 全仓门禁

`sh scripts/ci-local.sh` 退出 1，耗时 123 秒。Rust 1153 次执行通过；Desktop 104 项（103 通过、1 跳过）；Web 225 项（198 通过、11 失败、16 跳过）。总执行数 1482，与上一轮 M-04 实现证据基线 1482 差 0；1454 通过、11 失败、17 跳过也一致。Web 失败原因为 `listen EPERM: operation not permitted 127.0.0.1`，未绕过沙箱或改测试；其它门禁步骤通过。完整门禁不能声称绿色，需可监听环境复验。M-05 自身验收 5/5 通过，保留待审。

`sh -n plugins/autome/tests/evals.sh plugins/autome/tests/run.sh` 通过。最终原件全量复跑 59/59，日志 final-all.txt。
