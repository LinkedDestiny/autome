# 实现 #10：宿主门禁失败修复（关联 M-06）

## 范围与状态

本轮按修复轮指令，不推进新里程碑。六个里程碑已由审计关闭，保持原状态；implementation-round 更新为 10/30，current-milestone 与 next-action 保持无。上一审计无退回缺陷，本轮检查已在项目测试体系中，不需要搬入临时审计程序。

宿主原始日志：`/Users/dannie/project/autome-v2/.autome/output/gates/T-53/merge_gate-fa9d68c96246.log`，只读定位；失败索引保存到 `.autome/output/r10-failures.txt`。Rust e2e 177 个中 1 个失败；Web 538 个中 2 个失败。未运行全量门禁或宿主快检，最终结果由宿主重跑决定。

## 根因与修复

- Rust `bug_loop_task_get_reports_the_fix_round_for_bugs_only` 的完整字段清单缺少设计 §6 明确新增的 after、blocked_by。加入这两个字段，额外断言 feature/bug 默认值均为 []。保留完整字段比较、fix_round 类型差异、merge 默认对象及错误状态 reply 无副作用检查。
- Web dashboardDouble 缺少 §6 的 queued，导致真实 dashboard 顶层键比较失败。补齐 queued: []，并在真实账本用例断言空队列。保留原完整键比较、首次恰一条通知及重启去重检查。
- 第二个 Web 失败是第一个用例在 notifier.pull 前退出，导致第二个用例取得空通知基线；只补齐上述契约后即通过，未修改通知器。
- 同类检查：检索 Rust e2e 的字段全集与 Web dashboard 顶层键比较，匹配到本轮两处；dashboardDouble 的通知器消费者一并运行。无生产代码变更。设计 §10 逐项列明必要测试更新，原有行为断言不放宽。

## 红绿证据与计数

1. 红：`cargo test -p automed --test end_to_end bug_loop_task_get_reports_the_fix_round_for_bugs_only -- --nocapture`：1 个失败，字段差为 after、blocked_by。日志 `.autome/output/r10-rust-red.txt`。
2. 红：`npm test --prefix apps/web -- notifier-backfill-real`：4 个，2 失败、2 通过；与宿主两条失败一致。日志 `.autome/output/r10-web-red.txt`。
3. 绿：原 Rust 命令：1 个通过，0 跳过。日志 `.autome/output/r10-rust-green.txt`。
4. 扩展回归：`npm test --prefix apps/web -- notifier`：7 文件、47 个用例，44 通过、3 跳过。6 文件通过；notifier-live.test.ts 的 beforeAll 因 `listen EPERM: operation not permitted 127.0.0.1` 失败，3 个用例均未执行，整条命令不能报绿。日志 `.autome/output/r10-web-green.txt`。原失败的 backfill-real 4 个包含在通过项中；环境限制不改断言、不申请提权，交宿主复验。
5. M-06 验收：`npm test --prefix apps/web -- write-gate`：47 个通过；`! grep -n "排队按 created_at 升序" docs/development/technical-design.md && grep -q "queue move" README.md && grep -q "task.set_after" docs/development/technical-design.md` 返回 0。日志 `.autome/output/r10-write-gate.txt`。
6. 原 Web 失败文件独立复验：`npm test --prefix apps/web -- notifier-backfill-real` 返回 0，4 个全部通过。日志 `.autome/output/r10-web-backfill-green.txt`；这 4 个是上述 95 个中的重复执行，不增加去重总数。
7. `git diff --check` 通过。

最终选取 95 个去重用例（Rust 1 + notifier 47 + write-gate 47），92 通过、3 因上述环境限制未执行。相对宿主同一子集总数差 0：没有新增、删除测试用例，只增强原用例断言；相对实现 #9 的 49 个去重用例，选择范围增加 46，本轮换为门禁失败与直接相关回归子集，不能把差值当作新增测试数。M-06 自身仍为 47，与审计 #6 基线差 0。红阶段及诊断重复执行不重复计入用例总数。

## 标待审前自审四项（本轮保持既有关闭状态）

1. 情形表逐行：设计 §6 task.get 的 feature/bug 空依赖均为 []；dashboard 空 queued 为 [] 且保留旧顶层字段。设计 §8 set_after/move 放行、task_id 必填、位置互斥、创建/修改依赖列表校验，write-gate 47 个通过。通知器首次补齐、重启同状态不重发、驳回再进入等待、旧库升级在真实账本子集中通过。
2. 失败分支：Rust 错误状态 reply 拒绝且不写事件；write-gate 非法编号、非数组、超过 8 个、缺失/冲突位置参数全部由原用例检查。读盘失败、外部服务不可达不适用：本轮仅更新测试数据契约；真实 HTTP/live 检查受监听权限阻塞，明确未执行。
3. 全部迁移：本轮不修改产品状态机；相关通知器回归覆盖首次回填、关闭/重开、同状态事件去重、驳回后重入等待、旧库升级。live 的跨连接已读及重启推送迁移未取证，交宿主原用例复验。
4. 域边界：feature/bug 两种任务、空依赖数组、空队列；网关 8/9 项、空列表、单编号及非法类型边界通过。日期、最大整数等不适用：改动不处理这些输入域。

所有测试使用既有临时账本；未用本轮二进制打开真实账本。未启动下一会话。
