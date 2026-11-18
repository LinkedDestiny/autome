设计 #1 | M-01…M-11 | 设计初稿待评审（文档 54.6KB，首轮增长超 10KB 警告线） | docs/面板观感收尾/面板观感收尾.md | 无
实现 #1 | M-01 | 待审 | docs/面板观感收尾/evidence/M-01-r1-impl.md | 快检浏览器与关联测试受 ps/listen EPERM 阻塞
实现 #2 | M-01 | 待审 | docs/面板观感收尾/evidence/M-01-r2-impl.md | C-2 本机初始化 ps EPERM，待宿主复验
审计 #1 | M-01 | 通过 | docs/面板观感收尾/evidence/M-01-r2-audit.md | 无（C-2 浏览器项移入 H-10）
实现 #3 | M-02 | 待审；BL-01/02 纳入 M-12/13，BL-03 忽略 | docs/面板观感收尾/evidence/M-02-r3-impl.md | 快检 listen/ps EPERM
审计 #2 | M-02 | 通过 | docs/面板观感收尾/evidence/M-02-r3-audit.md | 无

实现 #4 | M-03 | 待审 | docs/面板观感收尾/evidence/M-03-r4-impl.md | 快检 e2e 初始化 ps EPERM，宿主复验
审计 #3 | M-03 | 通过 | docs/面板观感收尾/evidence/M-03-r4-audit.md | 无（抽屉浏览器用例由宿主 e2e 承担）

实现 #5 | M-04 | 待审 | docs/面板观感收尾/evidence/M-04-r5-impl.md | 快检监听与 ps 被沙箱拒绝，待宿主复验
审计 #4 | M-04 | 通过 | docs/面板观感收尾/evidence/M-04-r5-audit.md | 无

实现 #6 | M-05 | 待审 | docs/面板观感收尾/evidence/M-05-r6-impl.md | 快检真实 SSR/浏览器受沙箱权限限制，交宿主复验
审计 #5 | M-05 | 通过 | docs/面板观感收尾/evidence/M-05-r6-audit.md | 无

实现 #7 | M-06 | 待审 | docs/面板观感收尾/evidence/M-06-r7-impl.md | 浏览器与监听/ps 检查受沙箱限制
实现 #8 | M-06 | 待审 | docs/面板观感收尾/evidence/M-06-r8-impl.md | F-1 已更新范围链接；浏览器初始化受 ps EPERM 阻断，待宿主复跑
审计 #6 | M-06 | 通过 | docs/面板观感收尾/evidence/M-06-r8-audit.md | 无

实现 #9 | M-07 | 待审 | docs/面板观感收尾/evidence/M-07-r9-impl.md | 快检真实内核与浏览器清理受 ps EPERM 限制

实现 #10 | M-07 | 待审 | docs/面板观感收尾/evidence/M-07-r10-impl.md | 浏览器启动受 ps EPERM 限制，宿主复验
实现 #11 | M-07 | 待审 | docs/面板观感收尾/evidence/M-07-r11-impl.md | 本地监听与 ps EPERM，实际十条几何待宿主复验
实现 #12 | M-07 | 待审 | docs/面板观感收尾/evidence/M-07-r12-impl.md | 浏览器及实时测试受 ps/listen EPERM 阻塞，宿主复验
实现 #13 | M-07 | 待审，桌面区块间距收紧 | docs/面板观感收尾/evidence/M-07-r13-impl.md | 本地浏览器准备 ps EPERM，待宿主复验
审计 #7 | M-07 | 退回 + 任务详情时间线几何 | docs/面板观感收尾/evidence/M-07-r13-audit.md | 无

实现 #14 | M-07 | 待审 | docs/面板观感收尾/evidence/M-07-r14-impl.md | 浏览器 ps EPERM，宿主需以重建产物复验
审计 #8 | M-07 | 通过 | docs/面板观感收尾/evidence/M-07-r14-audit.md | 无

实现 #15 | M-08 | 待审 | docs/面板观感收尾/evidence/M-08-r15-impl.md | 快检内核清理及浏览器准备 ps EPERM，9 步预算未跑
审计 #9 | M-08 | 通过 | docs/面板观感收尾/evidence/M-08-r15-audit.md | 无

实现 #16 | M-09 | 待审 | docs/面板观感收尾/evidence/M-09-r16-impl.md | 快检 6 步受 ps/监听端口 EPERM 阻塞，待宿主复跑
审计 #10 | M-09 | 通过 | docs/面板观感收尾/evidence/M-09-r16-audit.md | 无

实现 #17 | M-10 | 待审 | docs/面板观感收尾/evidence/M-10-r17-impl.md | 浏览器 ps/监听权限受限，快检结果见证据
实现 #18 | M-10 | 待审，修复复制用例展开步骤，88 用例通过 | docs/面板观感收尾/evidence/M-10-r18-impl.md | 浏览器初始化 ps EPERM，待宿主复验
审计 #11 | M-10 | 通过 | docs/面板观感收尾/evidence/M-10-r18-audit.md | 无

实现 #19 | M-11 | 待审 | docs/面板观感收尾/evidence/M-11-r19-impl.md | 无；实际拍图保留 H-01
审计 #12 | M-11 | 通过 | docs/面板观感收尾/evidence/M-11-r19-audit.md | 无

实现 #20 | M-12 | 待审 | docs/面板观感收尾/evidence/M-12-r20-impl.md | 快检浏览器阶段 ps EPERM，待宿主复验
审计 #13 | M-12 | 通过 | docs/面板观感收尾/evidence/M-12-r20-audit.md | 无
实现 #21 | M-13 | 待审 | docs/面板观感收尾/evidence/M-13-r21-impl.md | 快检浏览器 ps EPERM，宿主复验
审计 #14 | M-13 | 通过 | docs/面板观感收尾/evidence/M-13-r21-audit.md | 无
实现 #22 | 无（修复轮） | 待审里程碑不变 | docs/面板观感收尾/evidence/fix-r22-impl.md | 无
实现 #23 | 无（交接轮） | 待审里程碑不变 | docs/面板观感收尾/evidence/M-13-r23-impl.md | 无
实现 #24 | 无（交接轮） | 待审里程碑不变 | docs/面板观感收尾/evidence/M-13-r24-impl.md | 无
