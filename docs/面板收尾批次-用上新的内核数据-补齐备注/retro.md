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
审计 #10 | M-08 | 通过 | evidence/M-08-r15-audit.md | 无（自造检查的 pkill 过宽，已披露）

实现 #16 | M-09 | 待审 | evidence/M-09-r16-impl.md | 关联 Web 的 listen/ps EPERM 交宿主复验
实现 #17 | M-09 | 待审 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-09-r17-impl.md | 无；修复驱动登记释放竞态，宿主复验快检
审计 #11 | M-09 | 通过 | evidence/M-09-r17-audit.md | 无

实现 #18 | M-10 | 开放（验收受阻） | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-10-r18-impl.md | 沙箱禁止 ps 与本地监听

实现 #19 | M-10 | 开放（快检修复，不推进） | evidence/M-10-r19-impl.md | 浏览器验收沿用上一轮待复验项
审计 #12 | M-10 | 通过 | evidence/M-10-r19-audit.md | 无

实现 #20 | M-11 | 实现落地，保持开放 | evidence/M-11-r20-impl.md | ps EPERM，真实进程验收交宿主

实现 #21 | M-11 | 快检超时修复，保持开放 | evidence/M-11-r21-impl.md | 真实进程验收仍受 ps 权限限制；宿主复跑快检
审计 #13 | M-11 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-11-r21-audit.md | 无

实现 #22 | M-12 | 开放（验收受限） | evidence/M-12-r22-impl.md | 浏览器 setup 的 ps EPERM，待宿主复验
审计 #14 | M-12 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-12-r22-audit.md | 无（浏览器两例移入人工清单 H-07）

实现 #23 | M-13 | 待审 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-13-r23-impl.md | 无
审计 #15 | M-13 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-13-r23-audit.md | 无

实现 #24 | M-05 | 门禁超时修复待复验 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-05-r24-impl.md | 浏览器启动被 ps EPERM 阻断

审计 #16 | M-13 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-13-r24-audit.md | 无（Playwright 受沙箱限制，由宿主门禁复验）

实现 #25 | M-04 | 待审（门禁修复，原状态不变） | evidence/M-04-r25-impl.md | 浏览器 ps EPERM，宿主复验
审计 #17 | M-13 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-13-r25-audit.md | 无（Playwright 受沙箱限制，由宿主门禁复验）

实现 #26 | M-04 | 待审（门禁修复，原状态不变） | evidence/M-04-r26-impl.md | ps EPERM，真实 fixture 耗时待宿主复验
审计 #18 | M-13 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-13-r26-audit.md | 无
实现 #27 | M-13 | 待审（集成修复，里程碑状态不变） | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-13-r27-impl.md | 浏览器初始化 ps EPERM，待宿主复验
审计 #19 | M-13 | 通过 | docs/面板收尾批次-用上新的内核数据-补齐备注/evidence/M-13-r27-audit.md | 无
