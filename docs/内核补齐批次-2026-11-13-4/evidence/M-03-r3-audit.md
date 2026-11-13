# M-03 · 审计复验（审计 #3，实现轮 r3）

独立复验，未读实现轮证据。

## 验收命令
`cargo test -p autome-domain test_fix_`：9 个通过（名单与契约一致），域库全量 477 个通过、0 失败。

## 负向对照（均已变红并还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| 上限判断 `>=` 改 `>` | `test_fix_limit_reached_fails_with_a_readable_reason` 红 |
| 声明缺陷后改走 `start_fix` | `test_fix_fix_declaring_a_defect_starts_locate_without_a_fix_round`、`test_fix_review_declaring_a_defect_starts_locate` 红 |
| 忽略 `bug_test_defect` 开关 | `test_fix_old_protocol_treats_the_key_as_unparseable` 红 |
| 自造：`judge_test_fix` 的 Accept 去掉 `!head_timed_out` | `test_fix_pair_judgement_accept_needs_fix_not_reproduced` 红 |

## 结论
通过。无产品缺陷、无验证缺口。临时备份在 `.autome/output/bug_loop.orig`（不入库）。
