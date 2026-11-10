设计 #1 | 无 | 设计初稿：M-01..M-04 | docs/在-apps-web-建-react/在-apps-web-建-react.md | 无
评审 #4 | M-01 | 发现 1 条六类问题，交裁决 | docs/在-apps-web-建-react/在-apps-web-建-react-review.md | D4-P01：Node 下限偏离任务
裁决 #1 | M-01 | 采纳 D4-P01（Node 下限回到 22，依赖降到 vite6/vitest3/eslint9+tsx）；设计定稿 | docs/在-apps-web-建-react/在-apps-web-建-react-adjudication.md | 无
设计 #2 | M-01 | 按用户驳回意见改为 Node≥22.22.0、RR8、vite8/vitest5/eslint10、原生类型剥离去 tsx，关 B-D1-01；status 回到设计中 | docs/在-apps-web-建-react/在-apps-web-建-react.md | 无
评审 #5 | M-01 | 通过 | docs/在-apps-web-建-react/在-apps-web-建-react-review.md | 无
裁决 #2 | M-01 | 评审 #5 无阻塞问题，无需裁决；设计定稿，status 改为实现中，design-round 2/15 | docs/在-apps-web-建-react/在-apps-web-建-react-adjudication.md | 无

实现 #1 | M-01 | 开放，验收阻塞 | docs/在-apps-web-建-react/evidence/M-01-r1-impl.md | 回环监听 EPERM、文件监听 EMFILE

实现 #2 | M-01 | Desktop 修复通过，保持开放 | docs/在-apps-web-建-react/evidence/M-01-r2-impl.md | Rust PID 清理断言阻塞完整门禁
审计 #1 | M-01 | 通过 | docs/在-apps-web-建-react/evidence/M-01-r2-audit.md | 无
实现 #3 | M-02 | 保留开放：专项通过，全量受阻 | docs/在-apps-web-建-react/evidence/M-02-r3-impl.md | 监听 EPERM、文件监听 EMFILE
审计 #2 | M-02 | 通过 | docs/在-apps-web-建-react/evidence/M-02-r3-audit.md | 无

实现 #4 | M-03 | 保持开放 | docs/在-apps-web-建-react/evidence/M-03-r4-impl.md | 双运行时通过；全量因沙箱 EPERM/EMFILE 阻塞
审计 #3 | M-03 | 通过 | docs/在-apps-web-建-react/evidence/M-03-r4-audit.md | 无

实现 #5 | M-04 | 环境阻塞，保留开放 | docs/在-apps-web-建-react/evidence/M-04-r5-impl.md | 本地监听 EPERM，文件监听 EMFILE
审计 #4 | M-04 | 通过 | docs/在-apps-web-建-react/evidence/M-04-r5-audit.md | 无
复盘 #1 | 无 | 写 5 条教训（L-01..L-05） | docs/在-apps-web-建-react/lessons.md | 无
