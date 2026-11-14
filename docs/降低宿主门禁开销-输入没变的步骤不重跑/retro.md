评审 #1 | M-01–M-06 | 退回 + 设计正确性 | 降低宿主门禁开销-输入没变的步骤不重跑-review.md | 6 项六类问题待裁决，1 项非阻塞建议

实现 #1 | M-01 | 待审 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-01-r1-impl.md | 无
审计 #1 | M-01 | 通过 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-01-r1-audit.md | 无

实现 #2 | M-02 | 待审 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-02-r2-impl.md | 无
审计 #2 | M-02 | 通过 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-02-r2-audit.md | 无
实现 #3 | M-03 | 待审 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-03-r3-impl.md | 无
审计 #3 | M-03 | 通过 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-03-r3-audit.md | 无

实现 #4 | M-04 | 待审 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-04-r4-impl.md | 无
审计 #4 | M-04 | 通过 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-04-r4-audit.md | 无
实现 #5 | M-05 | 待审 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-05-r5-impl.md | 各类完整快检耗时待宿主采集
审计 #5 | M-05 | 通过 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-05-r5-audit.md | 无

实现 #6 | M-06 | 保持开放 | evidence/M-06-r6-impl.md | 监听与 ps 权限阻塞；性能、连跑及 ENOTEMPTY 待取证
审计 #6 | M-06 | 退回 + Web 测试分组配置 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r6-audit.md | 无
实现 #7 | M-06 | 保持开放，分组启动已修复 | evidence/M-06-r7-impl.md | 监听与 ps 被拒绝，性能及五连跑未验收
审计 #7 | M-06 | 退回 + Web 内联样式规则 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r7-audit.md | 无

实现 #8 | M-06 | 样式回归修复，保持开放 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r8-impl.md | 监听与 ps 权限阻塞，性能及连续五次待验证
审计 #8 | M-06 | 退回 + Web 测试夹具清理竞态 | docs/降低宿主门禁开销-输入没变的步骤不重跑/evidence/M-06-r8-audit.md | 无
