设计 #1 | 无 | 设计初稿（M-01..M-05，新增用例 24 个，过滤前缀基线均为 0） | 无证据文件（设计轮） | 无
评审 #1 | M-01 | 发现 3 项问题，待裁决 | 费用可见-任务与项目累计费用进-task-review.md | 预算等额误报、告警去重写入窗口、项目查询列名歧义
实现 #1 | M-01 | 待审 | docs/费用可见-任务与项目累计费用进-task/evidence/M-01-r1-impl.md | 无
实现 #2 | M-01 | 待审 | docs/费用可见-任务与项目累计费用进-task/evidence/M-01-r2-impl.md | 门禁未通过：Electron SIGABRT、Web listen EPERM；待宿主复验
实现 #3 | M-01 | 待审 | docs/费用可见-任务与项目累计费用进-task/evidence/M-01-r3-impl.md | Desktop SIGABRT 未修复；当前环境禁止回环监听
审计 #1 | M-01 | 通过 | docs/费用可见-任务与项目累计费用进-task/evidence/M-01-r3-audit.md | 无
实现 #4 | M-02 | 待审 | docs/费用可见-任务与项目累计费用进-task/evidence/M-02-r4-impl.md | 无
审计 #2 | M-02 | 通过 | docs/费用可见-任务与项目累计费用进-task/evidence/M-02-r4-audit.md | 无
审计 #3 | M-03 | 无待审（未实现，表未改） | docs/费用可见-任务与项目累计费用进-task/evidence/M-03-r4-audit.md | 调度错位：需 M-03 实现轮
实现 #5 | M-03 | 待审 | docs/费用可见-任务与项目累计费用进-task/evidence/M-03-r5-impl.md | 无
审计 #4 | M-03 | 通过 | docs/费用可见-任务与项目累计费用进-task/evidence/M-03-r5-audit.md | 无
实现 #6 | M-04 | 待审 | docs/费用可见-任务与项目累计费用进-task/evidence/M-04-r6-impl.md | 无
审计 #5 | M-04 | 通过 | docs/费用可见-任务与项目累计费用进-task/evidence/M-04-r6-audit.md | 无
实现 #7 | M-05 | 环境阻塞，保持开放 | docs/费用可见-任务与项目累计费用进-task/evidence/M-05-r7-impl.md | Electron SIGABRT；Web listen EPERM / watch EMFILE
审计 #6 | M-05 | 通过 | docs/费用可见-任务与项目累计费用进-task/evidence/M-05-r7-audit.md | 无
复盘 #1 | 无 | 写 4 条教训（L-01..L-04） | docs/费用可见-任务与项目累计费用进-task/lessons.md | 无
实现 #8 | M-05 | rebase 完成，保留已审状态 | docs/费用可见-任务与项目累计费用进-task/evidence/M-05-r8-impl.md | 门禁队列测试失败及桌面/Web 验证阻塞
审计 #7 | 无 | 通过（无待审，变基后全量测试全绿） | docs/费用可见-任务与项目累计费用进-task/evidence/M-05-r8-audit.md | 无
复盘 #2 | 无 | 写 6 条教训（L-01..L-06，新增 L-05 门禁队列用例依赖 10 秒墙钟、L-06 审计未回应实现点名的失败） | docs/费用可见-任务与项目累计费用进-task/lessons.md | 无
实现 #9 | M-05 | rebase 完成，保留审计关闭状态 | docs/费用可见-任务与项目累计费用进-task/evidence/M-05-r9-impl.md | Desktop SIGABRT；Web EPERM/EMFILE；H-01–H-03 未取证
