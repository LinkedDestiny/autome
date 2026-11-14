设计 #1 | M-01..M-10（开放） | 设计初稿：A3/B3/C2/D2 共 10 个里程碑；实测面板脱仓运行、停面板门禁存活、Electron 43 跑服务、100 任务走查 | docs/面板交付批次-w10-启动与安装/面板交付批次-w10-启动与安装.md | 无
评审 #1 | M-01..M-10 | 提出 3 条六类问题，另列 4 条非阻塞建议 | docs/面板交付批次-w10-启动与安装/面板交付批次-w10-启动与安装-review.md | 生产模式、单实例互斥、薄壳停止归属
裁决 #1 | M-01..M-10 | 3 条评审问题全采纳并修正：serve 固定生产模式、serve 为唯一持锁层、stop 绑定 instance；N-01/03/04 采纳、N-02 入 Backlog；设计定稿 | docs/面板交付批次-w10-启动与安装/面板交付批次-w10-启动与安装-adjudication.md | 无
设计 #2 | M-11、M-12（新增，开放） | 按用户表态纳入 BL-02/03/04→M-11、BL-07→M-12；忽略不做：BL-01 驱动持有者、BL-05 README 截图、BL-06 开发服务登记、BL-08 导航复核 | docs/面板交付批次-w10-启动与安装/面板交付批次-w10-启动与安装.md | 无
评审 #2 | M-01..M-12 | 提出 1 条六类问题、3 条非阻塞建议 | docs/面板交付批次-w10-启动与安装/面板交付批次-w10-启动与安装-review.md | 健康超时接管仍活实例违反 A-2
裁决 #2 | M-01（修正）、M-12（措辞） | 采纳 R2-P01：A5 实例判定改三态（活/陈旧/无响应），只有确认失效才接管，无响应时 start/serve/status/stop 各有处置；M-01 增至 30 条；N2-01/02 采纳、N2-03 入 Backlog BL-09；设计保持定稿 | docs/面板交付批次-w10-启动与安装/面板交付批次-w10-启动与安装-adjudication.md | 无

实现 #1 | M-01 | 开放（环境阻塞） | docs/面板交付批次-w10-启动与安装/evidence/M-01-r1-impl.md | 沙箱禁止监听与 ps，21 条真实服务验收未通过
审计 #1 | M-01 | 退回 + launcher | docs/面板交付批次-w10-启动与安装/evidence/M-01-r1-audit.md | web-stop-gate 4 条失败：测试在门禁进程出现前取 pid
实现 #2 | M-01 | 退回 launcher（修复落地，保持开放；BL-09 纳入 M-13） | docs/面板交付批次-w10-启动与安装/evidence/M-01-r2-impl.md | ps EPERM，完整验收待复验
审计 #2 | M-01 | 通过 | docs/面板交付批次-w10-启动与安装/evidence/M-01-r2-audit.md | 无
实现 #3 | M-02 | 开放，环境阻塞 | docs/面板交付批次-w10-启动与安装/evidence/M-02-r3-impl.md | 3 条真实服务用例被 ps EPERM 阻塞
审计 #3 | M-02 | 通过 | docs/面板交付批次-w10-启动与安装/evidence/M-02-r3-audit.md | 无

实现 #4 | M-03 | 开放，环境阻塞 | docs/面板交付批次-w10-启动与安装/evidence/M-03-r4-impl.md | listen/ps EPERM，待宿主复验
审计 #4 | M-03 | 退回 + package | docs/面板交付批次-w10-启动与安装/evidence/M-03-r4-audit.md | 无

实现 #5 | M-03 | 待审 | docs/面板交付批次-w10-启动与安装/evidence/M-03-r5-impl.md | 2 条安装链路/残留检查受 ps EPERM 阻塞
审计 #5 | M-03 | 通过 | docs/面板交付批次-w10-启动与安装/evidence/M-03-r5-audit.md | 无

实现 #6 | M-04 | 待审 | docs/面板交付批次-w10-启动与安装/evidence/M-04-r6-impl.md | 无
审计 #6 | M-04 | 通过 | docs/面板交付批次-w10-启动与安装/evidence/M-04-r6-audit.md | 无
实现 #7 | M-05 | 待审 | docs/面板交付批次-w10-启动与安装/evidence/M-05-r7-impl.md | 额外进程残留检查受 ps EPERM 阻塞
审计 #7 | M-05 | 通过 | docs/面板交付批次-w10-启动与安装/evidence/M-05-r7-audit.md | 无

实现 #8 | M-06 | 待审 | docs/面板交付批次-w10-启动与安装/evidence/M-06-r8-impl.md | 验收无阻塞；扩大回归有两条既有失败，已复跑基线确认
审计 #8 | M-06 | 通过 | docs/面板交付批次-w10-启动与安装/evidence/M-06-r8-audit.md | 无

实现 #9 | M-07 | 退回 shell | docs/面板交付批次-w10-启动与安装/evidence/M-07-r9-impl.md | Electron SIGABRT、listen/ps EPERM；保持开放，reopen 不变
审计 #9 | M-07 | 退回 shell | docs/面板交付批次-w10-启动与安装/evidence/M-07-r9-audit.md | 无

实现 #10 | M-07 | 开放：纯测试通过，真实冒烟阻断 | docs/面板交付批次-w10-启动与安装/evidence/M-07-r10-impl.md | Electron SIGABRT、监听与 ps EPERM
