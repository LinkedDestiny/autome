设计 #1 | 无 | 设计初稿（M-01..M-04） | docs/服务端每次-tick-后用-events/服务端每次-tick-后用-events.md | 无
评审 #1 | 无 | 退回 + eventbus-resume/live-routes/nul-escape | docs/服务端每次-tick-后用-events/服务端每次-tick-后用-events-review.md | 3 条内部逻辑矛盾，待裁决

实现 #1 | M-01 | 保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r1-impl.md | npm registry DNS失败，ws及类型包未安装
实现 #2 | M-01 | 依赖声明修复，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r2-impl.md | registry DNS 失败且指定版本安装包未缓存
实现 #3 | M-01 | 门禁修复受阻，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r3-impl.md | registry ENOTFOUND，锁定包未缓存
实现 #4 | M-01 | 门禁修复受阻，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r4-impl.md | registry DNS 失败，锁定依赖安装失败
实现 #5 | M-01 | 门禁修复受阻，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r5-impl.md | registry DNS 失败，锁定依赖未安装
实现 #6 | M-01 | 退回 + eventbus-e2e（保持开放） | docs/服务端每次-tick-后用-events/evidence/M-01-r6-impl.md | registry DNS 不可达，锁定依赖无法安装

实现 #7 | M-01 | 开放（门禁修复受阻） | docs/服务端每次-tick-后用-events/evidence/M-01-r7-impl.md | registry DNS 失败，锁定依赖无完整缓存
实现 #8 | M-01 | 开放（门禁修复受阻） | docs/服务端每次-tick-后用-events/evidence/M-01-r8-impl.md | 在线 DNS 失败、锁定版本无离线缓存

实现 #9 | M-01 | 退回 + 依赖安装环境（保持开放） | docs/服务端每次-tick-后用-events/evidence/M-01-r9-impl.md | npm DNS 不通且锁定包无缓存
实现 #10 | M-01 | 退回 + eventbus-e2e（保持开放） | docs/服务端每次-tick-后用-events/evidence/M-01-r10-impl.md | 锁定依赖 DNS/缓存不可用
实现 #11 | M-01 | 依赖修复受阻，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r11-impl.md | npm DNS 失败且锁定包无缓存
实现 #12 | M-01 | 依赖修复受阻，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r12-impl.md | npm DNS 失败且锁定包无缓存
