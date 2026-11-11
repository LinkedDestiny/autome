# Autome 对话追问 eval

这三个场景评估技能第 1 步：缺验收标准先追问、Bug 缺复现步骤与期望行为一次追问、类型不明先问类型。
各 case.yaml 使用 runs、max_turns、timeout_seconds 限制运行；graders 引用同目录下的 Markdown 判分规则。
每个 grader 都有阳性对照：注入已知错误回复应判 0，并提供合格回复应判 1，避免无条件给分。

## 运行与成本

真实 eval 会拉起 Agent 并收费，不进门禁。只在修改触发描述、追问或模板规则后按需运行：

```sh
claude plugin eval ./plugins/autome --ablation with-without --max-cost-usd 2.00 --runs 3 --threshold 0.7 --json
```

三个场景均应达到 0.7。ablation 同时比较带插件和不带插件的表现；结构检查通过不代表真实 Agent 通过。
本任务的实际运行与判分对照复核留在人工验收 H-05。

## 工具限制

不要加 `--allow-tools Bash`，也不要开放其它 gated tools。正确回复都在整理与追问阶段停下，不需要命令。
不使用 scaffold_script，不需要夹具，也不要加 --scaffold。禁止为了 eval 向真实账本创建任务。
工具被拦截不能替代行为通过：grader 仍将任何命令尝试判 0。

## 确定性检查

```sh
sh plugins/autome/tests/run.sh evals
```

E1 检查三个场景的必需字段及 grader 引用；E2 检查全部 grader 的阳性对照；E3 检查验收标准判分；
E4 禁止 scaffold_script；E5 检查带成本上限的运行命令及上述工具限制。它们只验证结构与约束文本。
