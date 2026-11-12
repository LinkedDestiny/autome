设计 #1 | 无 | 设计初稿（M-01..M-03，cargo 基线 1193 通过，计划新增 12 + web 1） | 无证据文件（设计轮） | 无
评审 #1 | M-01 | 退回 + convert-core | docs/不是-bug-改建成需求-autome/不是-bug-改建成需求-autome-review.md | D1-P01 原子提交；D1-P02 取消完成判据
裁决 #1 | M-01 | 采纳 D1-P01、D1-P02；单事务改建，设计定稿待批准 | docs/不是-bug-改建成需求-autome/不是-bug-改建成需求-autome-adjudication.md | 无
实现 #1 | M-01 | 待审 | docs/不是-bug-改建成需求-autome/evidence/M-01-r1-impl.md | 宿主门禁 Electron DOM 退出、Web EPERM/EMFILE，需外部复验
审计 #1 | M-01 | 通过 | docs/不是-bug-改建成需求-autome/evidence/M-01-r1-audit.md | 无
实现 #2 | M-02 | 待审 | docs/不是-bug-改建成需求-autome/evidence/M-02-r2-impl.md | Desktop/Web 环境门禁未通过
审计 #2 | M-02 | 通过 | docs/不是-bug-改建成需求-autome/evidence/M-02-r2-audit.md | 无
实现 #3 | M-03 | 开放，网关通过但全量验收受阻 | docs/不是-bug-改建成需求-autome/evidence/M-03-r3-impl.md | 沙箱禁止本地监听，全量未完成
审计 #3 | M-03 | 通过 | docs/不是-bug-改建成需求-autome/evidence/M-03-r3-audit.md | 无
实现 #4 | M-03 | rebase 完成，保留审计关闭状态 | docs/不是-bug-改建成需求-autome/evidence/M-03-r4-impl.md | Web 监听 EPERM、Electron SIGABRT，需宿主复验
审计 #4 | M-03 | 通过 | docs/不是-bug-改建成需求-autome/evidence/M-03-r4-audit.md | 无
