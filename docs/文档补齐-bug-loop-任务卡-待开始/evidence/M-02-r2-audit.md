# M-02 审计复验（实现 #2 · 审计 #2）

结论：**通过**。

## 验收命令
`... | sh -s M-02` → `M-02 PASS 131/131`，退出码 0。

## 负向对照（均已还原，git diff 为空）
- 把 `task_briefs` 改名 → `缺 task_briefs`（130/131）。
- 8.2 标题改成 8.3 → 多出标题行、8.2 位置、缺 8.2 标题（128/131）。
- 把 `65536` 改成文字 → `缺 65536`。
- 改写已有行 `事件：`→`事件:` → 「删除或改写了 1 行」。
- 把 §9 新段的“必须是 1 到 20 的整数”改为“必须合法” → 仍 PASS，因为错误信息引文里还有同一字面：关键词检查对该词只有一份冗余，不是产品缺陷；语义由下面逐行核对覆盖。

## 验收 2 逐行核对（T-40..T-56，打开代码上下文）
- T-40 `dispatch.rs` 1341 只能是 feature 或 bug ✓
- T-41 必须字符串 / 去空白为空 / `len() > 65536` 三条信息与文档一致 ✓
- T-42 缺标题信息一致 ✓；T-43/T-44/T-46 测试名存在 ✓
- T-45 `task_briefs` 建表 ✓；T-47 `附件不存在：{a}`，`is_file` ✓
- T-48 `write_task_inputs`：已有卡不覆盖、附件仅 first_time 复制 ✓；T-49 提交信息一致 ✓
- T-50 `task_start`：Start → mark_queued → `task.started` → tick，返回 task/started/queue_position ✓
- T-51 取消草稿测试存在 ✓
- T-52 `fix_rounds` 1..=20 与信息一致 ✓；T-53 默认 3、`[bug]` 只在项目配置（`ProjectConfig.bug: Option`）✓
- T-54 `task.reply`：Bug Loop 任务空文本拒“补充信息不能为空”，否则 Reply；非 Bug Loop → `Reject{feedback: info}` ✓
- T-55/T-56 测试与函数存在 ✓
- 文档里引用的测试名、函数名（task_create/create/create_task/task_brief 等）均能在 crates 中找到。

## 项目测试体系
`sh scripts/ci-local.sh` exit 0，全部 ok（cargo test 各包 25/14/384/648/106/16/7 通过，0 失败）。

提交由本轮执行；未发现产品行为与文档不符。
