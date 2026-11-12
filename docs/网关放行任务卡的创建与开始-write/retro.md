设计 #1 | M-01、M-02 | 设计初稿 | docs/网关放行任务卡的创建与开始-write/网关放行任务卡的创建与开始-write.md | 无
评审 #1 | M-01、M-02 | 退回 + read-gate | docs/网关放行任务卡的创建与开始-write/网关放行任务卡的创建与开始-write-review.md | P-01：家目录符号链接可越界探测
裁决 #1 | M-01、M-02 | 设计定稿 | docs/网关放行任务卡的创建与开始-write/网关放行任务卡的创建与开始-write-adjudication.md | 无
设计 #2 | M-03、M-04 | 纳入 B1-02→M-03（正文逐字读回）、B1-03→M-04（替身 CLI 取证成功使用）；B1-01 用户决定忽略，不做 realpath 收紧 | docs/网关放行任务卡的创建与开始-write/网关放行任务卡的创建与开始-write.md | 无
评审 #2 | M-01、M-02、M-03、M-04 | 通过 | docs/网关放行任务卡的创建与开始-write/网关放行任务卡的创建与开始-write-review.md | 无
实现 #1 | M-01 | 保持开放 | docs/网关放行任务卡的创建与开始-write/evidence/M-01-r1-impl.md | 沙箱监听 EPERM，真实 HTTP 验收未取证
审计 #1 | M-01 | 通过 | docs/网关放行任务卡的创建与开始-write/evidence/M-01-r1-audit.md | 无
实现 #2 | M-02 | 退回 + read-gate（保持开放，环境未取证） | docs/网关放行任务卡的创建与开始-write/evidence/M-02-r2-impl.md | 监听 EPERM、文件监听 EMFILE

实现 #3 | M-02 | 退回 + 环境阻塞（保持开放） | docs/网关放行任务卡的创建与开始-write/evidence/M-02-r3-impl.md | 本地监听 EPERM，宿主原失败未复现
审计 #2 | M-02 | 通过 | docs/网关放行任务卡的创建与开始-write/evidence/M-02-r3-audit.md | 无
