设计 #1 | M-01–M-11 | 设计初稿（11 个里程碑，A–G；实测发现 set_loop 用 NoSkills 校验，修复列为对任务卡字面的取舍待评审） | docs/web-面板按动森风格重做/web-面板按动森风格重做.md | 无
评审 #1 | M-01–M-11 | 提出 6 项六类问题、3 项非阻塞建议 | docs/web-面板按动森风格重做/web-面板按动森风格重做-review.md | 待裁决；ps 扫描受沙箱限制
裁决 #1 | M-01–M-11 | 采纳 D1-P01…P06 全部六项，修改 B-Q1 / D-2 / D-3 / F-Q2 / F-Q3 与 M-03、M-05、M-09 验收；N1、N2 顺手修，N3 进 BL-06；设计定稿，status 改为实现中 | docs/web-面板按动森风格重做/web-面板按动森风格重做-adjudication.md | 等用户批准
实现 #1 | M-01 | 待审 | docs/web-面板按动森风格重做/evidence/M-01-r1-impl.md | 快检浏览器 ps EPERM；组件预览 Chromium 权限拒绝
审计 #1 | M-01 | 通过 | docs/web-面板按动森风格重做/evidence/M-01-r1-audit.md | 无

实现 #2 | M-02 | 待审；BL-01/06纳入M-12/13，BL-02/03/04/05忽略（用户裁定） | docs/web-面板按动森风格重做/evidence/M-02-r2-impl.md | ps/listen EPERM，浏览器待复验
审计 #2 | M-02 | 通过 | docs/web-面板按动森风格重做/evidence/M-02-r2-audit.md | 无

实现 #3 | M-03 | 开放（浏览器阻塞） | docs/web-面板按动森风格重做/evidence/M-03-r3-impl.md | 浏览器夹具 ps EPERM
审计 #3 | M-03 | 通过 | docs/web-面板按动森风格重做/evidence/M-03-r3-audit.md | 无

实现 #4 | M-04 | 保持开放 | docs/web-面板按动森风格重做/evidence/M-04-r4-impl.md | 浏览器进程检查 ps EPERM
审计 #4 | M-04 | 通过 | docs/web-面板按动森风格重做/evidence/M-04-r4-audit.md | 无

实现 #5 | M-05 | 待审 | docs/web-面板按动森风格重做/evidence/M-05-r5-impl.md | 快检浏览器 ps EPERM，待宿主复验
审计 #5 | M-05 | 通过 | docs/web-面板按动森风格重做/evidence/M-05-r5-audit.md | 无
实现 #6 | M-06 | 开放：单元通过，验收受阻 | docs/web-面板按动森风格重做/evidence/M-06-r6-impl.md | 沙箱 ps EPERM，浏览器未运行
审计 #6 | M-06 | 通过 | docs/web-面板按动森风格重做/evidence/M-06-r6-audit.md | 无

实现 #7 | M-07 | 保持开放 | docs/web-面板按动森风格重做/evidence/M-07-r7-impl.md | 浏览器夹具 ps EPERM
审计 #7 | M-07 | 通过 | docs/web-面板按动森风格重做/evidence/M-07-r7-audit.md | 无

实现 #8 | M-08 | 退回 + 验收环境 | docs/web-面板按动森风格重做/evidence/M-08-r8-impl.md | 浏览器与快检被 ps EPERM 阻断
审计 #8 | M-08 | 通过 | docs/web-面板按动森风格重做/evidence/M-08-r8-audit.md | 无
实现 #9 | M-09 | 退回 + 验收环境 | docs/web-面板按动森风格重做/evidence/M-09-r9-impl.md | 浏览器 ps EPERM，保持开放

实现 #10 | M-09 | 修复 F-1，保持开放 | docs/web-面板按动森风格重做/evidence/M-09-r10-impl.md | 浏览器 ps EPERM，待宿主复验
审计 #9 | M-09 | 通过 | docs/web-面板按动森风格重做/evidence/M-09-r10-audit.md | 无

实现 #11 | M-10 | 开放，Vitest 107 通过 | docs/web-面板按动森风格重做/evidence/M-10-r11-impl.md | 浏览器夹具 ps EPERM，5 条未执行

实现 #12 | M-10 | 快检修复通过，保持开放 | docs/web-面板按动森风格重做/evidence/M-10-r12-impl.md | M-10 浏览器五条待补验

审计 #10 | M-10 | 通过 | docs/web-面板按动森风格重做/evidence/M-10-r12-audit.md | 无

实现 #13 | M-11 | 开放，环境阻塞 | docs/web-面板按动森风格重做/evidence/M-11-r13-impl.md | ps EPERM，截图未生成
实现 #14 | M-11 | 退回 + 截图环境阻塞（保持开放） | docs/web-面板按动森风格重做/evidence/M-11-r14-impl.md | ps EPERM，缺少两图
审计 #11 | M-11 | 退回 + README截图交付 | docs/web-面板按动森风格重做/evidence/M-11-r14-audit.md | 无

实现 #15 | M-11 | 五图补齐，保持开放 | docs/web-面板按动森风格重做/evidence/M-11-r15-impl.md | ps EPERM；总览截图缺失值待修
审计 #12 | M-11 | 退回 + 总览缺值显示 | docs/web-面板按动森风格重做/evidence/M-11-r15-audit.md | 无
