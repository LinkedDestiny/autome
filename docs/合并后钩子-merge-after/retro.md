设计 #1 | 无 | 设计初稿（M-01..M-04，基线 1196 用例，计划新增 20） | 无证据文件（设计轮） | 无
评审 #1 | M-01 | 退回 + 子阶段与恢复设计 | docs/合并后钩子-merge-after/合并后钩子-merge-after-review.md | D1-P01、D1-P02
裁决 #1 | M-01 | 采纳 D1-P01、D1-P02（prepared 标记、事务写警报+结果、补开始事件）；设计定稿 → 实现中 | docs/合并后钩子-merge-after/合并后钩子-merge-after-adjudication.md | 无
实现 #1 | M-01 | 待审 | docs/合并后钩子-merge-after/evidence/M-01-r1-impl.md | 无
实现 #2 | M-01 | 待审 | docs/合并后钩子-merge-after/evidence/M-01-r2-impl.md | Electron SIGABRT；Web EPERM/EMFILE，门禁未绿
审计 #1 | M-01 | 通过 | docs/合并后钩子-merge-after/evidence/M-01-r2-audit.md | 无
实现 #3 | M-02 | 待审 | docs/合并后钩子-merge-after/evidence/M-02-r3-impl.md | 无
审计 #2 | M-02 | 通过 | docs/合并后钩子-merge-after/evidence/M-02-r3-audit.md | 无
实现 #4 | M-03 | 待审 | docs/合并后钩子-merge-after/evidence/M-03-r4-impl.md | 无
审计 #3 | M-03 | 通过 | docs/合并后钩子-merge-after/evidence/M-03-r4-audit.md | 无
实现 #5 | M-04 | 开放（环境阻塞） | docs/合并后钩子-merge-after/evidence/M-04-r5-impl.md | Electron SIGABRT；Web listen EPERM/EMFILE
审计 #4 | M-04 | 通过 | docs/合并后钩子-merge-after/evidence/M-04-r5-audit.md | 无
复盘 #1 | 无 | 写 4 条教训（L-01..L-04）；本任务无 Autome 指标，未引用数值 | docs/合并后钩子-merge-after/lessons.md | 无
实现 #6 | M-04 | 退回 + 集成复验 | docs/合并后钩子-merge-after/evidence/M-04-r6-impl.md | rebase 完成；钩子8项绿；Electron/Web环境受限，main队列全量曾失败
审计 #5 | M-04 | 通过 | docs/合并后钩子-merge-after/evidence/M-04-r6-audit.md | 无
复盘 #2 | 无 | 覆盖写 lessons.md 为 6 条（L-01..L-06；L-02/L-03 补入实现 #6 与审计 #5 的复现，新增 L-05 夹具依赖缺省配置、L-06 非本任务失败用例的干净 main 对照）；本任务无 Autome 指标，未引用数值 | docs/合并后钩子-merge-after/lessons.md | 无

实现 #7 | M-04 | 退回 + 验证环境 | docs/合并后钩子-merge-after/evidence/M-04-r7-impl.md | rebase 完成；Electron SIGABRT，Web EPERM/EMFILE
审计 #6 | M-04 | 通过 | docs/合并后钩子-merge-after/evidence/M-04-r7-audit.md | 无
复盘 #3 | 无 | 覆盖写 lessons.md 为 7 条（L-01..L-07；L-02/L-03 补入实现 #7 与审计 #6 的复现，新增 L-07：fail-fast 使审计 #6 的 cargo test 漏跑 end_to_end 等 177 项而证据未对基线总数）；本任务无 Autome 指标，未引用数值 | docs/合并后钩子-merge-after/lessons.md | 无
