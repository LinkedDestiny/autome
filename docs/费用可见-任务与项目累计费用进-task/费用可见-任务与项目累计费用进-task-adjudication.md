# 裁决记录

## 第 1 轮（design-round 0 → 1）

评审依据：`费用可见-任务与项目累计费用进-task-review.md`（评审 #1，基线 `a72691c`）。
评审提出 3 条六类问题（R1-P01..P03）与 2 条非阻塞建议（N-01、N-02）。本轮核对源码与反例后全部采纳，
设计已按下表修改。复提计数均为 0，没有争议项。

| ID | 主张摘要 | 设计位置 | 裁决 | 证据或理由 | 修改落点 | 复提计数 |
|---|---|---|---|---|---|---|
| D1-P01（R1-P01） | `usd_total*100.0 > limit_cents` 在浮点尾差下把等额费用判为超限（0.07/7 美分、0.10+0.20/30 美分） | 方案 §4.1 `budget_crossed` | 采纳 | 纯逻辑可证：`0.07*100.0 = 7.000000000000001` 为二进制浮点的已知结果；与任务 §6.6「严格大于」和设计表「累计=上限不告警」矛盾。评审附最小程序 `.autome/output/review-1-float.rs`。保留 `usd_total` 原始和，只在这一处比较加容差，不全局取整 | §4.1 公式加 `tolerance = 1e-6 + limit*1e-12` 与说明；情形表加尾差行与子美分超额行；M-04 第 1 个单测加三组金额；负向对照加「无容差」；风险表加一行 | 0 |
| D1-P02（R1-P02） | 警报、去重事件、通知事件分别提交，中途失败后重复报警 | 方案 §4.2 第 3 步 | 采纳 | 核对 `store.rs`：`raise_alert`（新 ID 独立 INSERT）、`append_event`、`last_event` 彼此独立，设计又规定写失败只记日志，故「警报已提交、去重事件缺失」可达，违反任务验收 3「一条警报」。`store.rs` 已有 `unchecked_transaction` 先例，无需新机制 | §4.2 改为 `store.raise_budget_alert` 单事务写三条；M-04 新增 lib 用例 `budget_alert_record_is_all_or_nothing`（删 `events` 表制造失败，断言无残留警报），预期 7→8，验收命令加 `cargo test -p automed --lib budget_alert_`；风险表、M-04 说明、负向对照同步 | 0 |
| D1-P03（R1-P03） | `SESSION_SELECT_BASE` + `JOIN tasks` 列名歧义 | 方案 §1.4 `sessions_in_project` | 采纳 | 核对 `store.rs:1877`：`id`、`kind`、`protocol_ref`、`rules_hash` 无表名前缀，`tasks` 有同名列，JOIN 在 prepare 阶段报 `ambiguous column name`；评审附复现 `.autome/output/feasibility-sql-join.txt` | §1.4 改为 `WHERE task_id IN (SELECT id FROM tasks WHERE project_id = ?1)` 并写明原因；M-02 说明与风险表同步 | 0 |
| D1-N01 | `status` 运行中行 `$3.75*` 没有脚注 | 方案 §2 | 不触发轮次，进 Backlog | 任务未要求；不属于六类 | Backlog B-05 | 0 |
| D1-N02 | `autome alerts` 打印原始 kind | 方案 §4.3 | 不触发轮次，进 Backlog | detail 已是完整中文，含义可读；不属于六类 | Backlog B-06 | 0 |

## 设计是否定稿

三条问题均已就地修正，修正后的方案无新引入的内部矛盾：`sessions_in_project` 的子查询形状与 `CostSummary::of` 分组不冲突；
容差只作用于 `budget_crossed`，不改变 `cost.usd_total` 的口径；事务只包住既有三次写入。
五个里程碑的验收命令、预期数、负向对照齐全，依赖无环。没有剩余六类问题，
状态块 `status` 由 `设计中` 改为 `实现中`，`design-round` 为 1/15，里程碑表 M-01..M-05 完整（均为 `开放`），等待用户批准。

提交：见本轮 git 提交；若被拒绝则由 Autome 兜底提交。
