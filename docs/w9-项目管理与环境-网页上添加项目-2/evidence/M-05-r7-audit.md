# M-05 审计 #6 复验（实现轮 r7）

- 验收命令（契约原文）：11 文件 73 例全过，skipped 0，与契约预期数一致（含实现轮受阻的 w9-http 7 例，本环境可监听）。
- 负向对照（均已 git checkout 还原，工作区干净）：
  1. 项目页 shouldRevalidate 改 bySubjectParam(['project.'],'id') → alerts 第 6 条与 alerts-real 第 1 条失败。
  2. projectAlerts 去掉 project_id 过滤 → alerts 第 1 条失败。
  3. loadShell 在 dashboard.get 失败时返回 kernel.ok=false → alerts 第 5 条失败。
- 读源码核对：action intent=dismiss 发 project.alert.dismiss；「知道了」随写门禁禁用；task_id 有值才链接；未知 kind 原文显示。
- 结论：通过，无实现缺陷，无验证缺口。
