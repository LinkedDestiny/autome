# 裁决记录

## 第 1 轮（design-round 0 → 1）

评审快照 `e0052fa`。评审结论：2 项阻塞意见（R1-01、R1-02），1 项非阻塞建议（N1-01）。两项阻塞意见均已核对源码并成立，已改设计；改后没有剩余的六类问题，设计定稿，`status` 改为 `实现中`。

### D1-P01（对应 R1-01）从标题派生的 request 仍进占位符渲染

- 主张：只给 brief 时 request 取自标题行，它被填进模板后再经 `{inputs}` 等替换与 `check_no_placeholders_left`，标题含 `{request}` 等文本会被改写或拒绝启动，违反 C7。
- 设计位置：方案 §5、§7 Q1。
- 裁决：**采纳**。
- 证据：`launcher.rs` 先替换 `{request}` 再替换其余占位符、最后检查剩余占位符；设计的「正文不进 prompt」只覆盖正文，不覆盖派生的 request。反例成立，评审附有实验日志。
- 修改落点：方案 §5——`card_path` 为 `Some` 时按 `{request}` 把模板切段，各段替换其余占位符并检查，request 原文最后拼入，不参与替换与检查；`card_path` 为 `None` 走今天的渲染，一字不改（C1）。M-03 新增 `a_request_with_placeholder_text_is_kept_literally_when_a_card_is_present`，lib 用例 4→5，M-05 总数 984→985。
- 复提计数：0。

### D1-P02（对应 R1-02）同秒开始旧草稿会插队

- 主张：`COALESCE(queued_at, created_at)` 加秒级时间戳，同一秒内旧草稿会排到先排队任务前面，违反 Q3「排在此前已经排队的任务之后」。
- 设计位置：方案 §2、§7 Q3；M-03 `task_card_a_started_draft_waits_behind_the_parallel_limit`。
- 裁决：**采纳**。
- 证据：`now_iso()` 是秒级格式；同秒时第一排序键相等，第二键 `created_at` 令旧草稿在前。评审在 SQLite 内存库复现。把时间戳换成亚秒精度也不可行（字符串比较下 `.900Z` 排在 `Z` 之前），所以改用整数序号。
- 修改落点：方案 §2——`queued_at` 改为 `queued_seq INTEGER`，`create_task`（非草稿）与 `mark_queued`（开始草稿）在单条语句内取 `MAX+1`；`queued_tasks` 按 `queued_seq IS NOT NULL, queued_seq, created_at` 排序，旧行（NULL）在前、顺序与今天相同。§3、Q3、M-01 与 M-03 的测试描述同步（M-01 用例加同一秒与两张草稿反序开始断言，用例数不变）。
- 复提计数：0。

### D1-N01（对应 N1-01）补写时区分「文件存在」与「已提交」

- 主张：写卡成功、`git add` 失败后重试，按「文件存在就跳过」会让卡片永远未提交。
- 裁决：**部分采纳**——设计本身没有规定跳过提交，不构成六类问题；作为实现注意事项进 Backlog。
- 修改落点：设计文档 Backlog `B-05`。
- 复提计数：0。

## 本轮结论

没有剩余的六类问题。里程碑表完整（M-01..M-05），验收契约齐全。`status` 改为 `实现中`，`design-round` 1/15。

提交由本轮尝试；若被拒绝，由 Autome 兜底提交。
