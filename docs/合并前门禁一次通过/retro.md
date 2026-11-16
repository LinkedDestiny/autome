设计 #1 | — | 设计初稿（7 个里程碑，A–E；快检内浏览器阶段 + 门禁日志标记段 + 内核退回放行） | docs/合并前门禁一次通过/合并前门禁一次通过.md | 无
评审 #1 | M-01 | 退回 + 设计评审 | docs/合并前门禁一次通过/合并前门禁一次通过-review.md | 7 项可追溯问题待裁决
裁决 #1 | — | 7 项评审意见全部采纳，设计定稿（status 实现中，design-round 1/15） | docs/合并前门禁一次通过/合并前门禁一次通过-adjudication.md | 无争议项
设计 #2 | — | 按用户驳回意见修改：撤销 D-1（删原 M-06，原 M-07 改为 M-06，共 6 个里程碑）；A-3 余量不足只在快检（CI_E2E_STAGE=quick）判失败，合并前门禁只报告 | docs/合并前门禁一次通过/合并前门禁一次通过.md | 无
评审 #2 | M-01 | 退回 + 设计评审 | docs/合并前门禁一次通过/合并前门禁一次通过-review.md | 1 项口径与沿用矛盾待裁决
裁决 #2 | — | R2-01 驳回（沿用全量门禁宽松记录是设计意图，写进 A-Q3 与 M-02 ⑨）；N2-01 采纳为文本；设计定稿（status 实现中，design-round 2/15） | docs/合并前门禁一次通过/合并前门禁一次通过-adjudication.md | 无争议项

实现 #1 | M-01 | 保持开放，验收未齐 | docs/合并前门禁一次通过/evidence/M-01-r1-impl.md | ps EPERM 阻断运行器清理与整套浏览器验收
实现 #2 | M-01 | 修复清理阻塞，保持开放 | docs/合并前门禁一次通过/evidence/M-01-r2-impl.md | ps EPERM；真实清理待宿主复验
审计 #1 | M-01 | 通过 | docs/合并前门禁一次通过/evidence/M-01-r2-audit.md | 无
实现 #3 | M-02 | 待审 | docs/合并前门禁一次通过/evidence/M-02-r3-impl.md | 全集复跑曾停滞中止，相关20条独立补跑通过
审计 #2 | M-02 | 通过 | docs/合并前门禁一次通过/evidence/M-02-r3-audit.md | 无

实现 #4 | M-03 | 待审 | docs/合并前门禁一次通过/evidence/M-03-r4-impl.md | 7 条真实 Playwright 测试 ps EPERM，交审计复验
审计 #3 | M-03 | 通过 | docs/合并前门禁一次通过/evidence/M-03-r4-audit.md | 无

实现 #5 | M-04 | 待审 | docs/合并前门禁一次通过/evidence/M-04-r5-impl.md | 9 条进程检查受 ps EPERM 阻断，交审计复验
审计 #4 | M-04 | 通过 | docs/合并前门禁一次通过/evidence/M-04-r5-audit.md | 无
实现 #6 | M-05 | 待审 | docs/合并前门禁一次通过/evidence/M-05-r6-impl.md | 无
实现 #7 | M-05 | 待审 | docs/合并前门禁一次通过/evidence/M-05-r7-impl.md | 宿主快检待重跑
审计 #5 | M-05 | 通过 | docs/合并前门禁一次通过/evidence/M-05-r7-audit.md | 无

实现 #8 | M-06 | 待审 | docs/合并前门禁一次通过/evidence/M-06-r8-impl.md | ★ 浏览器初始化 ps EPERM，宿主日志待审计引用

实现 #9 | M-06 | 待审 | docs/合并前门禁一次通过/evidence/M-06-r9-impl.md | 浏览器受 ps EPERM 阻止，待宿主复验
审计 #6 | M-06 | 退回 + 依赖事件验收判别力 | docs/合并前门禁一次通过/evidence/M-06-r9-audit.md | BL-01 相关分支恒假下仍通过
