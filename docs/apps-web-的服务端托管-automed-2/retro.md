设计 #1 | M-01..M-05 | 设计初稿（以附件为基线，去令牌与 Cookie，纳入 B-01；基线 Web 38、desktop 104、ci-local 全绿） | 设计文档「背景与现状」 | 无
评审 #1 | M-01..M-05 | 无六类阻塞问题，3 项非阻塞建议 | apps-web-的服务端托管-automed-2-review.md | Web 服务用例因 EPERM/EMFILE 未执行
裁决 #1 | M-01..M-05 | 评审无阻塞问题；3 项非阻塞建议已作为局部修正采纳（Vite CORS、等锁表述、B-03 描述）；设计定稿，status 改为实现中 | apps-web-的服务端托管-automed-2-adjudication.md | 无

实现 #1 | M-01 | 待审 | docs/apps-web-的服务端托管-automed-2/evidence/M-01-r1-impl.md | W1 的 7 个监听用例受 listen EPERM 阻塞
审计 #1 | M-01 | 通过 | docs/apps-web-的服务端托管-automed-2/evidence/M-01-r1-audit.md | 无
实现 #2 | M-02 | 待审 | docs/apps-web-的服务端托管-automed-2/evidence/M-02-r2-impl.md | W1 的 7 个监听用例受 EPERM/EMFILE 阻塞
