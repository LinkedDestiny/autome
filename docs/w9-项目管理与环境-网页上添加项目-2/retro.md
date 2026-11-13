设计 #1 | M-01..M-06（开放） | 设计初稿：6 个里程碑，真实内核实测 probe/add/Onboarding/报警/环境/配置回包 | docs/w9-项目管理与环境-网页上添加项目-2/w9-项目管理与环境-网页上添加项目-2.md | 无
评审 #1 | M-01..M-06 | 2 项问题待裁决 | docs/w9-项目管理与环境-网页上添加项目-2/w9-项目管理与环境-网页上添加项目-2-review.md | 补全完整输入校验、首次环境检测刷新

实现 #1 | M-01 | 保持开放 | docs/w9-项目管理与环境-网页上添加项目-2/evidence/M-01-r1-impl.md | HTTP 验收 listen EPERM；106/120 通过
审计 #1 | M-01 | 通过 | docs/w9-项目管理与环境-网页上添加项目-2/evidence/M-01-r1-audit.md | 无
实现 #2 | M-02 | 保持开放：55/59 通过 | docs/w9-项目管理与环境-网页上添加项目-2/evidence/M-02-r2-impl.md | 4 个 HTTP 用例监听 EPERM
审计 #2 | M-02 | 退回 + add-project | docs/w9-项目管理与环境-网页上添加项目-2/evidence/M-02-r2-audit.md | w9-http 新用例 post 辅助函数解析纯文本 403 报 JSON 错
