设计 #1 | M-01..M-05（开放） | 设计初稿 | docs/w5-外壳与项目导航-迁移-animal/w5-外壳与项目导航-迁移-animal.md | 无
评审 #1 | M-01..M-05（开放） | 退回 + 数据恢复与搜索字段 | docs/w5-外壳与项目导航-迁移-animal/w5-外壳与项目导航-迁移-animal-review.md | 2 条内部逻辑矛盾，交裁决
裁决 #1 | M-01..M-05（开放） | 采纳 2 条 + 更正 1 条，设计定稿 | docs/w5-外壳与项目导航-迁移-animal/w5-外壳与项目导航-迁移-animal-adjudication.md | 等待批准

实现 #1 | M-01 | 待审 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-01-r1-impl.md | 全量监听 EPERM/EMFILE；desktop 缺 Electron
审计 #1 | M-01 | 通过 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-01-r1-audit.md | 无

实现 #2 | M-02 | 待审 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-02-r2-impl.md | 监听 EPERM，Electron 缺失；需审计环境复验
审计 #2 | M-02 | 通过 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-02-r2-audit.md | 无
实现 #3 | M-03 | 待审 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-03-r3-impl.md | 全量监听环境受限，待审计复验
实现 #4 | M-03 | 待审 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-03-r4-impl.md | Rust 失败未复现；Web 监听 EPERM，待 Autome 复跑
审计 #4 | M-03 | 通过 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-03-r4-audit.md | 无

实现 #5 | M-04 | 待审 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-04-r5-impl.md | 监听 EPERM；Rust 端到端一次失败后单跑通过；Desktop harness 失败
审计 #5 | M-04 | 通过 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-04-r5-audit.md | 无

实现 #6 | M-05 | 待审 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r6-impl.md | 监听 EPERM/EMFILE；Electron SIGABRT
审计 #6 | M-05 | 通过 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r6-audit.md | 无
