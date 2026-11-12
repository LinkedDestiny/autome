# 宿主门禁按机器排队：同一时刻只跑 N 个门禁作业，排队不计预算、不计超时
## 背景
门禁作业（Gate / MergeGate / MainCheck / ReproCheck / FixGate / 准备命令）由宿主以后台作业运行，互不排队。并行 3 时经常 2–3 个完整 `ci-local`（fmt+clippy+全部 Rust 测试+e2e+desktop+web）同时跑，再叠加 3 个会话各自的 `cargo test`。实测（11-12）：T-28 合并后验证 main 时，T-36 的合并前门禁早 43 秒启动、T-35 的门禁早 20 分钟启动；Rust `Test` 步骤安静时 51 秒，这次 1192 秒（23 倍），整条门禁撞到 `[gate] timeout_minutes = 30` 被杀（退出码 124）。同一时段 T-36 的合并前门禁也超时，退回修复轮，白白消耗一个修复轮次；T-28 则留下一条 `main_gate_failed` 警报。证据：`.autome/output/gates/T-28/main_check-9b5750b07fe9.log`、`.autome/output/gates/T-36/merge_gate-fe76a37af7a5.log`、`.autome/output/gates/T-30/main_check-*.log`（安静基线）。
## 要做什么
1. 全局配置 `~/.autome/config.toml` 新增 `[gate] concurrency = 1`（可省，缺省 1；项目配置可覆盖但不得超过全局）。宿主同一时刻最多运行 N 个门禁/准备作业（跨项目计）。
2. 超过时任务停在原节点、状态显示「排队等门禁（前面还有 k 个）」；排队不启动作业、不写 `gate.started`、不计入任何预算；轮到时才启动，**超时时钟从作业真正启动时算**。顺序先到先得；MainCheck（合并后验证）与 MergeGate 可以优先于普通 Gate，设计轮定。
3. `autome status`/`run` 与 `dashboard.get`、`task.get` 体现排队状态（新事件 `gate.queued {position}`，或在 `task.get` 里加 `gate_queue_position`）。
4. core 重启后排队顺序按任务进入排队的时间恢复（可由事件推导，不加表）。
5. 顺带：门禁超时（124）产生的失败在 `FailureReason`/注入文案里明确写「门禁超时」，与测试失败区分（T-28 的熔断指纹已把超时单列，沿用）。
## 不做什么
不改 `ci-local.sh`；不改会话沙箱；不自动调 `timeout_minutes`。
## 验收标准
e2e：`concurrency = 1` 下两个任务同时到门禁，第二个只在第一个结束后才有 `gate.started`，期间 `task.get` 显示排队、无预算变化；超时按各自启动时间计；未配置时行为等同 `concurrency = 1`（这是缺省，所以要同时断言旧用例不变）。`cargo test --workspace` 全绿。README 配置一节补 `concurrency`。
## 参考
`crates/automed/src/gate.rs`、`scheduler.rs` 的 `run_gate`、T-27 的 `setup.rs`（同一机制）、`crates/automed/src/config_io.rs`（`[gate]` 解析）。
