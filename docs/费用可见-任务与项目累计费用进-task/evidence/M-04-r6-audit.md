# M-04 r6 审计复验

结论：通过。

1. 验收命令：9 个用例通过（3+2+2+1+1），基线 0。
2. 负向对照（注入→结果→还原）：`>=` 无容差→红；无容差→红；去重 `!=`→红；不去重→红（单测、e2e）；无事务→红；超限置 Stopped→e2e 红。保留容差的 `>=` 为等价变异。
3. 代码核对：`check_budget` 挂在 Onboarding 早退之后，错误只 warn；`raise_budget_alert` 单事务写警报与两条事件；去重读 `task.budget_exceeded` 事件的 `limit_cents`；CLI 与 Web 文案与设计一致。
4. 全量门禁：只红 B-07（已知，M-05 修）。
