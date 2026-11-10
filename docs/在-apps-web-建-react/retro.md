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
实现 #6 | M-04 | 锁释放回归通过，原状态保留 | docs/在-apps-web-建-react/evidence/M-04-r6-impl.md | Web 测试受沙箱 EPERM/EMFILE 阻塞，宿主门禁待复验
实现 #7 | M-04 | 修复两项时序测试，保持原状态 | docs/在-apps-web-建-react/evidence/M-04-r7-impl.md | Web 监听受沙箱 EPERM/EMFILE 阻塞，宿主待复验
审计 #5 | M-04 | 通过 | docs/在-apps-web-建-react/evidence/M-04-r7-audit.md | 无
复盘 #2 | 无 | 覆盖写 7 条教训（L-01..L-07，新增 L-06、L-07） | docs/在-apps-web-建-react/lessons.md | 无

## 终止总结

- 终止状态：`已完成`。M-01..M-04 均经审计关闭，审计 #5 在 c0a59e7 上复验宿主门禁 ci-local 退出 0。
- 轮次：设计 2/15（设计 #1 定稿后被用户在卡点驳回，设计 #2 重定稿）；实现 7/20。实现 #1、#3、#4、#5 各推进一个里程碑；实现 #2、#6、#7 未推进里程碑，修的是范围外门禁失败（Desktop Electron 加载时机、driver_lock 锁释放、gate/git 时序测试），依据用户专项指令越出硬性约束 1。
- 各里程碑：M-01..M-04 全部 `已完成`，reopen 均为 0，无重复领域；收敛模式始终 `normal`。关闭轮次：M-01 审计 #1、M-02 审计 #2、M-03 审计 #3、M-04 审计 #4（审计 #5 复验通过）。
- 实现缺陷 0，验证缺口 0。
- 预算：设计、实现均未超预算。
- 未完成部分：无功能阻塞。实现轮会话的回环监听 EPERM 始终未解除，Web server 套件只在审计与宿主环境里跑通。
- 人工验收未确认：H-01（dev 模式中文占位页与 HMR）、H-02（生产模式页面显示且无 hydration 报错）。

协议改进建议（证据见 lessons.md）：
1. 增加「实现完成、仅验收被执行环境阻断」时标 `待审` 的规定（L-02，evidence/M-02-r3-impl.md）。
2. 第一个实现轮前在基线提交上跑一次宿主门禁，失败项标为「基线失败」写进简报（L-04，evidence/M-01-r2-impl.md）。
