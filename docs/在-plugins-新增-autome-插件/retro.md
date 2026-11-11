设计 #1 | M-01..M-05 | 设计初稿 | 无（事实核对原始输出在 .autome/output/plan1/） | 无
评审 #1 | M-01..M-05 | 退回 + 确认与回显 | docs/在-plugins-新增-autome-插件/在-plugins-新增-autome-插件-review.md | P01 确认前 heredoc 执行风险；P02 排队回显错误

实现 #1 | M-01 | 待审 | docs/在-plugins-新增-autome-插件/evidence/M-01-r1-impl.md | 全量 CI：监听 EPERM，Electron 缺失
审计 #1 | M-01 | 通过 | docs/在-plugins-新增-autome-插件/evidence/M-01-r1-audit.md | 无
实现 #2 | M-02 | 待审；B-1～B-3 纳入 M-06～M-08，B-4 裁定忽略不做 | docs/在-plugins-新增-autome-插件/evidence/M-02-r2-impl.md | 全量环境限制见证据
审计 #2 | M-02 | 通过 | docs/在-plugins-新增-autome-插件/evidence/M-02-r2-audit.md | 无

实现 #3 | M-03 | 待审 | docs/在-plugins-新增-autome-插件/evidence/M-03-r3-impl.md | 全量 Web test 受沙箱监听权限限制
审计 #3 | M-03 | 通过 | docs/在-plugins-新增-autome-插件/evidence/M-03-r3-audit.md | 无

实现 #4 | M-04 | 待审 | docs/在-plugins-新增-autome-插件/evidence/M-04-r4-impl.md | 全仓 Web 测试受沙箱监听限制，与上一轮一致
审计 #4 | M-04 | 通过 | docs/在-plugins-新增-autome-插件/evidence/M-04-r4-audit.md | 无

实现 #5 | M-05 | 待审 | docs/在-plugins-新增-autome-插件/evidence/M-05-r5-impl.md | Web 监听 EPERM；真实 eval 留 H-05
审计 #5 | M-05 | 通过 | docs/在-plugins-新增-autome-插件/evidence/M-05-r5-audit.md | 无
实现 #6 | M-06 | 待审 | docs/在-plugins-新增-autome-插件/evidence/M-06-r6-impl.md | 无（全仓门禁环境限制见证据）
审计 #6 | M-06 | 通过 | docs/在-plugins-新增-autome-插件/evidence/M-06-r6-audit.md | 无
实现 #7 | M-07 | 待审 | docs/在-plugins-新增-autome-插件/evidence/M-07-r7-impl.md | 全仓 Web 测试受沙箱 listen EPERM 阻塞
审计 #7 | M-07 | 通过 | docs/在-plugins-新增-autome-插件/evidence/M-07-r7-audit.md | 无
