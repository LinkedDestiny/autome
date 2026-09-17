# loop 插件 agent-eval（T5）

模仿 mesh 的 agent-eval，用原生 `claude plugin eval` 针对**真实 agent 执行任务文件时的协议遵从性**评估。
每个场景为 `evals/<scenario>/case.yaml`（内联 prompt + `scaffold_script` 造夹具）+ `graders/*.md`（判分器）。

## 触发策略与成本（C-20，关键）

- **触发式 / 人工，禁止列为每轮必跑门禁**：`claude plugin eval` 真实拉起 agent、有成本，且当前处于 early access（本机 `claude plugin eval` 输出 `plugin eval is currently in early access`）。
- 运行时**必须**设成本硬上限并用无插件基线对照：

```bash
claude plugin eval ./plugins/loop \
  --ablation with-without \
  --max-cost-usd 2.00 \
  --runs 3 --threshold 0.7 \
  --json
```

- `--max-cost-usd`：命中即中止并报告部分结果（exit 2），成本有界。
- `--ablation with-without`：附带无插件基线臂，报告分差（防止「无插件也能过」的空评估）。
- 日常门禁不跑本层；由 `skill/testing/run-gates.sh`（T1–T3）的**确定性低成本替代**兜底，映射见 `coverage-matrix.md`。

## 场景集（三类协议遵从性）

| 场景目录 | 协议不变量 | 角色 |
|---|---|---|
| `reviewer-independence/` | 评审轮形成全部结论前不读裁决记录与旧评审 | AT1 |
| `auditor-rerun/` | 审计轮复验验收命令而非信任 `待审`（实现缺陷退回 `开放` 计 reopen） | AT4 |
| `launch-and-return/` | 每轮结束「启动即结束」不等待/轮询 | AT1–AT4 |

## Grader 纪律（C-22）

- 机检 grader 优先（deterministic：检查 transcript 中的工具调用序列 / 文件读取顺序 / 是否阻塞等待）。
- 使用 LLM grader 时**必须带阳性对照**：每个 grader.md 内含一个「应判负样本」，先确认 grader 能对它判负，再用于评分。
- `--judge-model` 默认 haiku；评分不稳定时提高 `--runs` 取多次均值。

## 早期访问说明

`claude plugin eval` 当前 early access，case.yaml/grader 字段以本机 `claude plugin eval --help` 记载为准
（`case.yaml` 或 `prompt.md` + `graders/*.md`；`runs`/`max_turns`/`timeout_seconds`/`scaffold_script`/`tags`）。
本目录为按该格式撰写的场景集；正式可用后按 `--help` 校订字段。G-05 的证据是本场景集与覆盖矩阵的存在性 +
确定性替代的绿灯，不把真实评估纳入每轮门禁。
