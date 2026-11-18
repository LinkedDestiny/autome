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
