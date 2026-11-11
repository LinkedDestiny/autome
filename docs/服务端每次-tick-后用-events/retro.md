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
实现 #13 | M-01 | 依赖修复受阻，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r13-impl.md | registry DNS 不可用且锁定包未缓存
实现 #14 | M-01 | 依赖修复受阻，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r14-impl.md | registry DNS 失败且锁定包未缓存
实现 #15 | M-01 | 门禁失败复现，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r15-impl.md | registry DNS 失败且锁定包未缓存
实现 #16 | M-01 | 门禁失败复现，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r16-impl.md | registry DNS 失败且锁定包未缓存
实现 #17 | M-01 | 门禁失败复现，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r17-impl.md | registry DNS 失败，依赖无法安装
实现 #18 | M-01 | 门禁失败复现，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r18-impl.md | registry DNS 失败，锁定依赖无法安装
实现 #19 | M-01 | 退回 + eventbus-e2e（保持开放） | docs/服务端每次-tick-后用-events/evidence/M-01-r19-impl.md | 沙箱监听 EPERM；依赖错误已不复现
实现 #20 | M-01 | 退回 + eventbus-e2e（保持开放） | docs/服务端每次-tick-后用-events/evidence/M-01-r20-impl.md | 已修正旧 404 前提；监听 EPERM 阻断集成复验，另一宿主失败待完整日志

实现 #21 | M-01 | 退回 + eventbus-e2e | docs/服务端每次-tick-后用-events/evidence/M-01-r21-impl.md | 本地监听 EPERM，保持开放
实现 #22 | M-01 | 修复取消事件嵌套状态断言，保持开放 | docs/服务端每次-tick-后用-events/evidence/M-01-r22-impl.md | 完整门禁受监听 EPERM 阻断，待宿主复跑
审计 #1 | M-01 | 通过 | docs/服务端每次-tick-后用-events/evidence/M-01-r22-audit.md | 无

实现 #23 | M-02 | 待审 | docs/服务端每次-tick-后用-events/evidence/M-02-r23-impl.md | 27 个 Web 用例受监听 EPERM 阻断，需审计复验
审计 #2 | M-02 | 通过 | docs/服务端每次-tick-后用-events/evidence/M-02-r23-audit.md | 无
