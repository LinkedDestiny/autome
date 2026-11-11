# 实现审计 · 审计 #4

M-04：通过。验收命令自己跑：e2e 96、automed lib 617 全过；过滤 `setup` e2e 16；README 与 technical-design 各 2 处 `[setup]`；`CONFIG-UNCHANGED`。
`scripts/ci-local.sh`：Format、Clippy、End to end、Adapter flags、Desktop、Contract、Web lint/typecheck/test/build 全部 ok（本环境此前的 Web/Desktop 受阻不复现）；唯一红的 `Test` 步是 `gate::tests::a_gate_that_runs_too_long_...` 一次 `Vanished`，这是基线就有的测试竞速（只等 pid 文件存在、不等内容），单跑 4/4 通过，lib 全量单跑 617 通过，记 Backlog B-06，不计缺陷。
负向对照（都已还原，工作区干净）：主检出准备忽略记录 → E11 红；去掉主检出互斥 → L3 红；`setup.skipped` 去重去掉 → E13 红。
证据：docs/项目可配置准备命令-依赖变化时由宿主在/evidence/M-04-r5-audit.md
