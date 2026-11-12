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
复盘 #1 | M-01..M-05（已完成） | 教训 4 条 | docs/w5-外壳与项目导航-迁移-animal/lessons.md | 无
实现 #7 | M-05 | 门禁调查未闭合，原状态保留 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r7-impl.md | 原 CLI 超时未复现；Rust 偶发断言失败
实现 #8 | M-05 | 门禁未闭合，保留原状态并补阶段诊断 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r8-impl.md | notifier-live 重启超时；本地监听 EPERM 阻断复现

实现 #9 | M-05 | 退回 Desktop 门禁调查，原状态保留 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r9-impl.md | Electron 启动 SIGABRT，宿主超时未闭合

实现 #10 | M-05 | 门禁修复候选，原状态保留 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r10-impl.md | 本地监听 EPERM，提交后由 Autome 复验
审计 #7 | 无 | 通过（无待审项，表不变；宿主门禁 5b394e1 全绿） | docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r10-audit.md | 无
复盘 #2 | M-01..M-05（已完成） | 教训 6 条（新增 L-05 门禁误派、L-06 分阶段诊断） | docs/w5-外壳与项目导航-迁移-animal/lessons.md | 无
实现 #11 | M-05 | 门禁定位，原状态保留 | docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r11-impl.md | 内核 PID 发布竞态；任务禁止改 crates，未修复
审计 #8 | 无 | 通过（无待审项，表不变；宿主门禁 9ff58fe 全绿） | docs/w5-外壳与项目导航-迁移-animal/evidence/M-05-r11-audit.md | 无
复盘 #3 | M-01..M-05（已完成） | 教训 7 条（新增 L-07 范围外失败未入 Backlog；L-05 补 b44b444 证据） | docs/w5-外壳与项目导航-迁移-animal/lessons.md | 无
