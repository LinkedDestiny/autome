设计 #1 | — | 设计初稿（9 个里程碑，A–F 实测后定稿待评审） | docs/面板收尾批次-用上新的内核数据-补齐备注/面板收尾批次-用上新的内核数据-补齐备注.md | 无
评审 #1 | M-01 | 退回 + 设计一致性（6 条待裁决） | docs/面板收尾批次-用上新的内核数据-补齐备注/面板收尾批次-用上新的内核数据-补齐备注-review.md | 分页、日志窗口、Onboarding 兜底、驱动语义、进程身份与未知路径告警
设计 #2 | M-09–M-13 | 按驳回意见改 F：有控制终端不杀（实测 e_tdev）；纳入 BL-01/04/10/11 为 M-10–M-13；BL-02/03/05/06/07/08/09/12 用户表态不做 | docs/面板收尾批次-用上新的内核数据-补齐备注/面板收尾批次-用上新的内核数据-补齐备注.md | 无
评审 #2 | M-05 | 退回 + URL 状态一致性（1 条待裁决） | docs/面板收尾批次-用上新的内核数据-补齐备注/面板收尾批次-用上新的内核数据-补齐备注-review.md | 窗口固定与文档导航互相覆盖参数
裁决 #2 | M-05 | 采纳 R2-P01，设计定稿（design-round 2，13 个里程碑） | docs/面板收尾批次-用上新的内核数据-补齐备注/面板收尾批次-用上新的内核数据-补齐备注.md | 查询参数互相独立（withParams）
实现 #1 | M-01 | 待审 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-01-r1-impl.md | ps/listen/Chromium 沙箱限制，交审计复验
实现 #2 | M-01 | 待审 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-01-r2-impl.md | related 的 ps/listen EPERM 交宿主复验
审计 #1 | M-01 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-01-r2-audit.md | 无

实现 #3 | M-02 | 待审 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-02-r3-impl.md | 浏览器及监听测试受沙箱 ps/listen EPERM 限制，宿主复验

实现 #4 | M-02 | 待审 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-02-r4-impl.md | 无；快检最终以宿主重跑为准
审计 #2 | M-02 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-02-r4-audit.md | 无

实现 #5 | M-03 | 开放（环境阻塞） | evidence/M-03-r5-impl.md | ps/listen EPERM；浏览器未执行，related 清理未通过
实现 #6 | M-03 | 快检修复，保持开放 | evidence/M-03-r6-impl.md | HTTP listen EPERM，浏览器验收待复验
审计 #3 | M-03 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-03-r6-audit.md | 无
审计 #3 | M-03 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-03-r6-audit.md | 无

实现 #7 | M-04 | 待审 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-04-r7-impl.md | ps/listen EPERM，浏览器三项与相关环境测试交审计复验
审计 #4 | M-04 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-04-r7-audit.md | 无

实现 #8 | M-05 | 待审 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-05-r8-impl.md | ps/listen EPERM，浏览器与清理交宿主复验
审计 #5 | M-05 | 退回 + 详情窗口浏览器验收夹具 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-05-r8-audit.md | 无

实现 #9 | M-05 | 待审 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-05-r9-impl.md | 浏览器 ps EPERM，交审计复验
审计 #6 | M-05 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-05-r9-audit.md | 无
实现 #10 | M-06 | 开放，D-1…D-4 已实现，待补证 | evidence/M-06-r10-impl.md | ps EPERM 阻断浏览器与 D-5 诊断
审计 #7 | M-06 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-06-r10-audit.md | 无

实现 #11 | M-07 | 退回 + 截图验收 | evidence/M-07-r11-impl.md | ps 被沙箱拒绝，三张旧图尚未替换；保持开放

实现 #12 | M-07 | 退回 + 截图生成受限 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-07-r12-impl.md | ps EPERM，真实截图未替换
审计 #8 | M-07 | 退回 + README 截图产物未替换 | evidence/M-07-r12-audit.md | 无

实现 #13 | M-07 | 待审 | evidence/M-07-r13-impl.md | 本轮 ps EPERM；采用审计已生成原图，生成流程待独立复验
审计 #9 | M-07 | 通过 | evidence/M-07-r13-audit.md | 无
实现 #14 | M-08 | 待审 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-08-r14-impl.md | ps/listen EPERM，真实面板交审计复验
实现 #15 | M-08 | 待审 | evidence/M-08-r15-impl.md | 本地快检无起点仅静态通过，宿主复跑为准
