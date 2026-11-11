# 审计 #6 · M-06

结论：通过。证据：docs/在-plugins-新增-autome-插件/evidence/M-06-r6-audit.md

新造检查：对 README 的 task-new-json 示例做七种键/值篡改（id、state、started、kind、branch、queue_position、archived），V7 全部失败；结果均符合预期，已还原。
