# M-04 · r6 · 审计复验

结论：**通过**。

## 负向对照（先跑）
- README 去掉 `AUTOME_MERGE_COMMIT` → grep 计数 0，验收的该项会失败。已还原（只是管道，未改文件）。
- 在 `.autome/config.toml` 追加一行 → `git diff --exit-code <merge-base> -- .autome/` 非零，不打印 `AUTOME-DIR-UNCHANGED`；`git checkout` 已还原，工作区干净。

## 验收命令（自己跑）
- `[merge]` 计数：README 2、technical-design 3；`AUTOME_MERGE_COMMIT` 在 README 1 处；`.autome/` 相对 merge-base 无改动（merge-base 即 main HEAD 90802f6，已 rebase 到 main 上）。
- README 示例块（L121–123）与条目 **After merge**（L193）已直接读过，均在。
- `cargo test --workspace --all-targets`：第二次全绿（exit 0），共 1385 用例 = 35+16+411+1(忽略)+745+142+28+7，1384 通过 1 忽略（t14_gate_logs）。用例数高于设计预期的 1219，差额来自 rebase 带进的 main 用例。
- `sh scripts/ci-local.sh`：exit 0，Format / Clippy / Test / Desktop / Contract / Web lint / typecheck / test / build 全部 ok（626s）；Electron 与 Web 在本会话环境可跑，实现轮 #6 的环境阻塞不复现。

## 第一次全量里的一个失败
首次全量 `scheduler::tests::gate_queue_upgrade_collects_two_old_jobs_without_killing_them` 失败（scheduler_gate_queue_tests.rs:225）。该用例属 main 的门禁队列，不触碰本任务新增代码路径（merge_after 仅在 Cleanup 且配了 `[merge] after` 时生效，该用例未配）。因果判别：在 `git archive main` 的干净副本里单跑同一用例 4+5 次，有 2 次失败；在本分支同样会间歇失败，且后续整库全量通过。结论：main 上既有的时序性 flaky，与本里程碑无关，记入 Backlog B-08。
