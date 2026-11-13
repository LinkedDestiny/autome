# M-03 审计 #3 独立复验

结论：通过。

## 验收命令
- `cargo test -p automed --test end_to_end kernel_adopt`：3 个通过。
- `cargo test -p automed --lib kernel_adopt`：1 个通过。合计 4，与契约一致。

## 负向对照（均已 `git checkout` 还原，工作区干净）
| 注入 | 结果 |
|---|---|
| `ctx.adopted_older_kernel |= adopted` 改成丢弃（去掉每进程一次） | e2e 2 个失败（once、advances） |
| 接管分支里的 `kernel.adopted` 事件整段禁用 | e2e 3 个全失败 |
| `AUTOME_ALLOW_OLDER_KERNEL` 把 `true` 也当真 | 库单测失败 |
（第一次事件注入写得太粗，替换了全部 append_event 导致编译失败，不算数；改为只禁用接管块后得到上表结果。）

## 自造检查
- 读代码：`Ctx::new` 默认 `allow_older_kernel=false`，环境只在 `apply_kernel_env`（main、CLI Client）读取；`AUTOME_TEST_BUILD_ID` 仅 debug 构建、仅能往旧改。
- 接管在 `claim_kernel_mark_with_adoption` 的 IMMEDIATE 事务内改 mark 并写事件；已有单测 `adoption_event_failure_rolls_back_mark` 守住回滚。
- 相邻子集：`--lib -- adoption kernel` 8 个、`--test end_to_end kernel` 10 个（含 M-02）全绿。
- 宿主快检（fmt/clippy/web lint/typecheck）在 552b6d6 通过。
无实现缺陷，无验证缺口。
