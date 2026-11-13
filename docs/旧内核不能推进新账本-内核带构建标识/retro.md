设计 #1 | 无 | 设计初稿（6 个里程碑，安装对齐选 autome core，账本记录走 v8 迁移） | 无 | 无
评审 #1 | 无 | 发现 4 项六类问题，交裁决 | docs/旧内核不能推进新账本-内核带构建标识/旧内核不能推进新账本-内核带构建标识-review.md | 排序、无锁推进、构建缓存、升级顺序
裁决 #1 | 无 | 采纳 D1-P01～P04 与 S-01/S-02；设计定稿（status→实现中，design-round 1/15；次序键改祖先提交数、取锁出错不推进、build.rs 缺失引用兜底、上线顺序先停旧进程；M-01 17→20、M-02 9→10 用例） | 旧内核不能推进新账本-内核带构建标识-adjudication.md | 无
实现 #1 | M-01 | 待审 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-01-r1-impl.md | 无
审计 #1 | M-01 | 通过 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-01-r1-audit.md | 无
实现 #2 | M-02 | 待审；B-01/02/04/05裁定忽略不做，B-03纳入M-07 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-02-r2-impl.md | 无
审计 #2 | M-02 | 通过 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-02-r2-audit.md | 无
实现 #3 | M-03 | 待审 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-03-r3-impl.md | 无
审计 #3 | M-03 | 通过 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-03-r3-audit.md | 无
实现 #4 | M-04 | 待审 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-04-r4-impl.md | 无
审计 #4 | M-04 | 通过 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-04-r4-audit.md | 无

实现 #5 | M-05 | 退回 web | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-05-r5-impl.md | 沙箱禁止本地监听，验收两例 EPERM，保持开放
审计 #5 | M-05 | 通过 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-05-r5-audit.md | 无

实现 #6 | M-06 | 待审 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-06-r6-impl.md | 无
审计 #6 | M-06 | 通过 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-06-r6-audit.md | 无

实现 #7 | M-07 | 待审 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-07-r7-impl.md | 无
实现 #8 | M-07 | 待审 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-07-r8-impl.md | 无
审计 #7 | M-07 | 退回 + package-isolation | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-07-r8-audit.md | 无

实现 #9 | M-07 | 待审 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-07-r9-impl.md | 无
审计 #8 | M-07 | 通过 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-07-r9-audit.md | 无
实现 #10 | M-02 | 通过（rebase 集成回归，保留审计状态） | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-02-r10-impl.md | 无
实现 #11 | M-06 | 门禁回归修复，保留已审状态 | evidence/M-06-r11-impl.md | 宿主快检与全量门禁待重跑

实现 #12 | M-05 | rebase 完成，保留已审状态 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-05-r12-impl.md | 2 例监听 EPERM，待宿主复验
审计 #9 | 无 | 无待审里程碑（M-01~M-07 均已完成，表未动） | docs/旧内核不能推进新账本-内核带构建标识/brief/audit-9.md | 无
实现 #13 | M-05 | 门禁夹具修复，保留已审状态 | docs/旧内核不能推进新账本-内核带构建标识/evidence/M-05-r13-impl.md | HTTP 九例监听 EPERM，待宿主复验
