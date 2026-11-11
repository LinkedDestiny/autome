设计 #1 | M-01..M-05 | 设计初稿 | .autome/output/w4-events/（真实内核账本 156 条事件，不入库） | 无
评审 #1 | M-01..M-05 | 退回 + 连接/导航/系统通知 | docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器-review.md | 3 项可追溯问题待裁决
裁决 #1 | M-01..M-05 | 采纳 D1-P01..P03，设计定稿 | docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器-adjudication.md | 无争议项，status=实现中
设计 #2 | M-01..M-05 | 按用户驳回意见修订：系统通知改 tag 合并、live-client 加 onMessage、规则加 not_a_bug、纳入 B-04 | .autome/output/w4-not-a-bug/（真实内核 not_a_bug 事件，不入库） | 无，status 回到设计中待评审

实现 #1 | M-01 | 待审 | docs/实现-web-sqlite3-与通知器/evidence/M-01-r1-impl.md | Web 监听回归受沙箱 EPERM/EMFILE 阻断
审计 #1 | M-01 | 通过 | docs/实现-web-sqlite3-与通知器/evidence/M-01-r1-audit.md | 无
实现 #2 | M-02 | 待审；B-01/B-04纳入M-06/M-07；B-02/B-03忽略不做 | docs/实现-web-sqlite3-与通知器/evidence/M-02-r2-impl.md | Web监听EPERM/EMFILE，交审计复验
审计 #2 | M-02 | 通过 | docs/实现-web-sqlite3-与通知器/evidence/M-02-r2-audit.md | 无

实现 #3 | M-03 | 待审 | docs/实现-web-sqlite3-与通知器/evidence/M-03-r3-impl.md | listen EPERM，live 与安全负向对照待复验
实现 #4 | M-03 | 待审 | docs/实现-web-sqlite3-与通知器/evidence/M-03-r4-impl.md | live/全量监听受 EPERM 阻断，夹具回归红绿已取证
审计 #3 | M-03 | 通过 | docs/实现-web-sqlite3-与通知器/evidence/M-03-r4-audit.md | 无

实现 #5 | M-04 | 待审 | docs/实现-web-sqlite3-与通知器/evidence/M-04-r5-impl.md | 30 个既有监听用例受 EPERM 等限制，视觉验收未取证
审计 #4 | M-04 | 通过 | docs/实现-web-sqlite3-与通知器/evidence/M-04-r5-audit.md | 无

实现 #6 | M-05 | 待审 | docs/实现-web-sqlite3-与通知器/evidence/M-05-r6-impl.md | 全量30例受监听权限阻断；真实通知保留人工验收
审计 #5 | M-05 | 通过 | docs/实现-web-sqlite3-与通知器/evidence/M-05-r6-audit.md | 无

实现 #7 | M-06 | 待审 | docs/实现-web-sqlite3-与通知器/evidence/M-06-r7-impl.md | 全量监听 EPERM / watcher EMFILE，交审计复验
审计 #6 | M-06 | 通过 | docs/实现-web-sqlite3-与通知器/evidence/M-06-r7-audit.md | 无
实现 #8 | M-07 | 待审 | docs/实现-web-sqlite3-与通知器/evidence/M-07-r8-impl.md | 全量遇 listen EPERM / watcher EMFILE
审计 #7 | M-07 | 通过 | docs/实现-web-sqlite3-与通知器/evidence/M-07-r8-audit.md | 无
实现 #9 | M-07 | 门禁原失败未复现，状态不变 | docs/实现-web-sqlite3-与通知器/evidence/M-07-r9-impl.md | Web listen EPERM / watcher EMFILE；待宿主复跑
实现 #10 | M-02 | 真实账本超时修复，状态不变 | docs/实现-web-sqlite3-与通知器/evidence/M-02-r10-impl.md | 完整门禁待宿主复验，范围外 Rust 超时及本地监听限制
审计 #8 | 无待审 | 通过（全部已完成，web 301 用例全过） | docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器-audit.md | 无
