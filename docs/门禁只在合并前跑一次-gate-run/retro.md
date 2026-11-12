设计 #1 | 无 | 设计初稿（M-01..M-05，基线 domain 385 / lib 650 / e2e 112 / 金样 16，计划新增 25） | 无证据文件（设计轮） | 无
评审 #1 | 无 | 发现 3 条六类问题，待裁决 | 门禁只在合并前跑一次-gate-run-review.md | 命令来源缺失可漏验、Bug 复核提示缺失、each_round 兼容范围
设计 #2 | 无 | 按用户驳回意见扩范围到 protocol/prompts：新增 M-05 分段渲染与宿主门禁句、M-06 协议 v3 与 eval，B-02 纳入，各里程碑验收改为子集，status 回 设计中 | 无证据文件（设计轮） | 无
评审 #2 | 无 | 发现 3 条六类问题，待裁决 | 门禁只在合并前跑一次-gate-run-review.md | 启动清掉复现标记、命令误当占位符、旧协议 Bug 复核缺证说明
裁决 #2 | 无 | 3 条全部采纳：标记不入 Job::clear、门禁命令按数据隔离、旧 Bug 复核补一句事实；设计定稿，status 转 实现中 | 门禁只在合并前跑一次-gate-run-adjudication.md | 无
设计 #3 | 无 | 处理用户表态：B-02 纳入，单列 M-07（bug-protocol 一句与 G-6），文档顺延为 M-08；B-01（桌面端路由图门禁框）、B-03（「每个任务一次」的限定）忽略不做，移出 Backlog；status 保持 实现中 | 无证据文件（设计轮） | 无
评审 #3 | 无 | 通过：无六类阻塞问题，2 条非阻塞建议；状态块与 8 个验收契约结构通过 | 门禁只在合并前跑一次-gate-run-review.md | 无
裁决 #3 | 无 | 无六类问题；N3-01 订正两处章节引用，N3-02 作为 M-06 实施提醒；设计保持定稿，status 保持 实现中，design-round 3/15 | 门禁只在合并前跑一次-gate-run-adjudication.md | 无
实现 #1 | M-01 | 待审 | docs/门禁只在合并前跑一次-gate-run/evidence/M-01-r1-impl.md | 无；原有 T-14 日志测试默认忽略
审计 #1 | M-01 | 通过 | docs/门禁只在合并前跑一次-gate-run/evidence/M-01-r1-audit.md | 无
实现 #2 | M-02 | 待审 | docs/门禁只在合并前跑一次-gate-run/evidence/M-02-r2-impl.md | 无
审计 #2 | M-02 | 通过 | docs/门禁只在合并前跑一次-gate-run/evidence/M-02-r2-audit.md | 无
实现 #3 | M-03 | 待审 | docs/门禁只在合并前跑一次-gate-run/evidence/M-03-r3-impl.md | 无
审计 #3 | M-03 | 通过 | docs/门禁只在合并前跑一次-gate-run/evidence/M-03-r3-audit.md | 无
实现 #4 | M-04 | 待审 | docs/门禁只在合并前跑一次-gate-run/evidence/M-04-r4-impl.md | 无
审计 #4 | M-04 | 通过 | docs/门禁只在合并前跑一次-gate-run/evidence/M-04-r4-audit.md | 无
