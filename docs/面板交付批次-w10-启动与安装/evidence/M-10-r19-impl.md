# M-10 · 实现 #19

结论：M-10 保持开放，reopen 2、domain-review 不变。本轮不能取得通过证据，不标待审。未修改产品、夹具、协议或测试断言。

## 审计回归与诊断

审计 #17 的判别检查逐字为：`cd apps/web && /usr/bin/time -p npm run e2e`。该检查已经是项目 npm e2e 测试入口，20 条用例及 170 秒 globalTimeout 原样保留，无需复制成第二套测试。

修改前原样执行，预检通过，setup 在 spawnSync ps 处报 EPERM，teardown 同样报错；真实命令退出失败，real 0.78 秒。日志：`.autome/output/r19-red.log`。这是环境红灯，不是审计性能缺陷的复现，也没有修复后的绿灯。进程扫描位于 prepare/spawn 前，因此未启动被测服务、内核或浏览器。未绕过扫描或增加条件跳过。

检查假 CLI 来源 w7-flow.sh、flowSteps、两套服务、flows/live 全部等待上限及 ticker。需求脚本本身没有固定 sleep；Bug 的 release 等待用于逐轮可见性；D-4 的重启延迟用于离线窗口，不能为了缩时删掉。内核既有 design-loop 测试说明 design→review→adjudicate→await_design_approval 是必要路径。没有证据证明可以直接删除需求的前置步骤。审计日志只有总时限和 13 条通过记录，未记录 D-3 各节点耗时，无法区分调度节奏与额外阻塞。不能据此声称根因已经确定。

## 命令、数量与基线

- 原验收命令：环境失败，20 条浏览器用例均未进入执行；日志 r19-red.log。
- `cd apps/web && npx vitest run test/task-timeline.test.ts test/e2e-preflight.test.ts`：18 条全部通过，0 失败、0 跳过；日志 `.autome/output/r19-subset.log`。
- `cd apps/web && npx playwright test --list`：4 文件 20 条；日志 `.autome/output/r19-list.log`。
- 用例总数 **38 = 18 单测 + 20 浏览器**；与实现 #18 同范围基线相比 **差 0**，没有增删用例。实际执行 18 条，20 条因 setup 权限失败未运行，不能算通过或主动跳过。审计 #17 浏览器基线为 13 通过、7 未运行，本轮仍发现 20 条，但没有获得可比较的性能测量。
- 未跑全量门禁、宿主快检或无关测试；没有进行未经验证的性能修复。

## 标待审之前的四项自审

1. **情形表逐行**：D-1 网页草稿→开始→批准→合并完成；D-2 CLI Bug 定位→复现检查→修复→复核→合并；D-3 CLI 批准五秒内换列；D-4 离线横幅与恢复补齐。四条均随原验收尝试执行，但全部在 setup 被阻塞。D-7 的 20 条发现数确认，总耗时与残留验收未通过。
2. **失败分支**：缺浏览器、缺 build/内核、齐全的预检三条通过；ps 不可用明确失败。D-4 内核异常退出恢复及三条负向对照（整页 reload、禁用 events.since 重放、D-3 改成 1000ms tick）未执行：基准浏览器套件尚不能启动，不能把环境失败当作负向对照成功。拒绝授权、外部服务不可达不属于本轮夹具性能问题，不适用。
3. **全部迁移**：需求 todo→running→needs_you→running→await_merge→done；Bug locate→repro_check→fix→fix_gate→fix_review→await_merge→done；网页/CLI→账本→事件→页面；在线→离线→恢复。现有浏览器检查保留，本轮均受阻。时间线节点投影与事件缺失→到达的既有单测通过，不替代端到端迁移证据。
4. **域边界**：既有时间线空事件/缺 kind/旧 node/无节点依据等边界随子集通过。3000ms tick、5000ms 更新上限、170 秒总限、180 秒验收上限均未放宽，实际时限行为未验证。未引入新输入域，前导零和最大值不适用。

## 后续与提交

需在允许 ps、监听与浏览器的环境运行原验收，采集 D-3 的节点耗时再压缩夹具；20 条全绿、real ≤180 秒、残留断言及三条负向对照取得证据后才能报审。环境阻塞不计协议失败，不增加 reopen，不启动下一会话。提交若被权限拒绝，提交由 Autome 兜底。
