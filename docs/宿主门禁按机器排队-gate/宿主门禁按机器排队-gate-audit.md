# 审计 · 第 1 轮

M-01：**通过**，标 `已完成`。证据：`docs/宿主门禁按机器排队-gate/evidence/M-01-r1-audit.md`。

- 验收命令全绿（domain 393、automed --lib 655）；三条负向对照均使对应用例变红，已还原。
- 验证缺口（已闭合，不计 reopen）：clippy `items_after_test_module` 在验收命令之外，已把 `parse_gate_concurrency` 移到测试模块前。
- 新造检查：三处缺陷注入（见证据），通过，丢弃。
- 提示：契约里 domain 基线 385 实为 384（总数 393 而非 394），M-02 起以实测基线为准。
