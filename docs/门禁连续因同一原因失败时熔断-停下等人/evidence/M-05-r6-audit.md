# M-05 r6 独立复验

## 负向对照（先做，已还原，git status 干净）
- render.rs 把 `"gate_same_failure"` 分支改名：`cargo test -p autome-cli gate_same_failure` → `explains_the_reason_and_log` FAILED（1 过 1 败）。
- labels.js 把 `case 'gate_same_failure'` 改名：`npm test --prefix apps/desktop` 新用例失败，actual 为原始 `gate_same_failure`。
- 旧测试删行：`numstat` 判据见下，删行只落在产品代码。

## 验收命令（自跑）
- `cargo test -p autome-cli gate_same_failure`：2 通过。
- 桌面端：105 项，104 过、0 失败（1 项跳过，非本里程碑）；新用例含 count 1/3/10。
- `cargo test --workspace`：合计 1130 通过 / 0 失败 / 1 忽略，与预期一致。
- clippy、fmt 无输出。
- `grep -c same_failure_limit README.md` = 2；`grep -c 熔断 technical-design.md` = 1。
- `numstat`：end_to_end.rs 159/0，labels-failure.test.js 17/0，删除列均 0。
- 含测试文件删行：仅 task.rs 1 行（第 963 行，`cfg(test)` 在 1435 行之后）与 scheduler.rs 11 行（run_gate，约 2848–2860 行，首个测试模块在 3576 行）——均在产品代码内，不在测试模块。

## 结论
通过。H-01 保留在人工验收清单，由用户合并前确认。
