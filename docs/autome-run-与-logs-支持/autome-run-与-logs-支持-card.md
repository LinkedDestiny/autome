# `autome run` 与 `autome logs` 支持 `--json`
## 背景
CLI 方案要求所有命令支持 `--json`，Agent 才能程序化驱动。`crates/autome-cli/src/main.rs:149-150` 的参数白名单里 `run` 和 `logs` 没有 `--json`。
## 要做什么
1. `run --json`：每个事件输出一行 JSON（`{seq, time, task_id, kind, payload}`），tick 错误输出 `{"error": ...}`；`--until-idle` 结束时输出一行 `{"idle": true, "tasks": [...]}`。不打人类可读文案。
2. `logs --json [--follow]`：每行一个 JSON（会话日志的结构化事件，沿用 `render-stream` 的解析），`--follow` 持续输出。
3. `tests/cli.rs` 各加一条用例。
## 不做什么
不改事件内容；不改 `render-stream`。
## 验收标准
两条命令在 `--json` 下 stdout 可被 `jq -c .` 逐行解析；`cargo test -p autome-cli` 全绿。
