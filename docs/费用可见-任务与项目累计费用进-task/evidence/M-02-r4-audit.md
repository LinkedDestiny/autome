# M-02 r4 审计复验

- 验收命令 `cargo test -p automed --lib cost_rollup_`：3 通过。
- 负向对照（均已还原）：
  1. 只把 `Done` 当已完成 → 用例 1 失败，其余通过；
  2. `sessions_in_project` 的 WHERE 改成恒真 → 用例 2 失败；
  3. `task_json` 调用方传空汇总 → 用例 1、3 失败。验收能拦住三类错误实现。
- 全量：`cargo test -p automed -p autome-domain`：lib 651、domain 388 通过；`end_to_end` 103 通过、1 失败：
  `bug_loop_task_get_reports_the_fix_round_for_bugs_only` 钉死了 task 对象的键集合，M-02 按设计给 `task_json` 加了
  `cost_usd` / `cost_sessions_without_cost`，该断言过时。产品行为符合设计 → 验证缺口，本轮把两个键加进基线，复验通过，不计 reopen。
- 桌面/Web 门禁属 M-05 范围，本轮未跑。
