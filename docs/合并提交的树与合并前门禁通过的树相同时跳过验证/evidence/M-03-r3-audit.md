# M-03 r3 审计复验

1. 负向对照（render.rs `event_line`）：(a) 守卫 `if p["node"]=="main_check"` 换成 `if false` → `main_check_reuse_is_printed_by_autome_run` 失败（15 过 1 败）；(b) 换成 `if true`（对所有 gate.reused 生效）→ 同用例失败（merge_gate 原句断言）。已还原，`git status` 干净。
2. 验收命令：`cargo test -p autome-cli render::` 16 通过；`grep -c` README=2、technical-design=1；`cargo test --workspace` rc=0，1223 通过/0 失败/1 ignored。
3. 读 diff：README `[gate]` 示例含 `reuse_for_main_check = true`，Host gate 说明句写明树相同+head+命令匹配才沿用、可设 false；technical-design §8.1 同步；w4-ledger.sh 夹具显式 `reuse_for_main_check = false` 保住原报警断言（符合 M-03 范围）。
4. `sh scripts/ci-local.sh`：Format/Clippy/Test/Contract/Web lint·typecheck·test·build ok；Desktop 因 electron dom harness 60s 超时（全量并行负载下）失败一次；单独 `node --test test/*dom*.test.js` 62/62、`npm test --prefix apps/desktop` 166/166 通过。本任务未触及 apps/desktop，判为环境负载抖动。
5. 结论：通过，M-03 标 已完成；三个里程碑全部关闭，status 置 已完成。
