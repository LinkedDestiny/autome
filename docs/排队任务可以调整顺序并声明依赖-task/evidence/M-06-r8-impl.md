# M-06 · 实现轮 8 · 待审

## 范围与修复

- 放行 `task.set_after`、`task.move`；数组白名单允许创建与修改依赖的 `after`。
- 两个新操作要求非空字符串 `task_id`；创建/修改依赖按输入项数限 8，编号格式为 `^T-\d+$`；移动要求 before/after/top=true 恰好一个，位置编号为字符串，top 为布尔值。
- 更新 README 命令行用法及 technical-design §8、§13、§14 的调度、v6、RPC、投影和事件。
- 上轮 M-05 审计通过，无退回缺陷，因此没有需逐字搬入并先红后绿的审计检查。
- 设计中“三处 id 列表”修正为两处依赖列表与一处移动位置编号；task.move.after 是字符串，不是第三个列表。

## 验收与计数

1. 改动前：`npm test --prefix apps/web -- write-gate`，44/44 通过。日志 `.autome/output/m06-baseline.txt`。
2. 验收：`npm test --prefix apps/web -- write-gate && ! grep -n "排队按 created_at 升序" docs/development/technical-design.md && grep -q "queue move" README.md && grep -q "task.set_after" docs/development/technical-design.md`，退出码 0；47/47 通过，三条文档检查成立。日志 `.autome/output/m06-acceptance.txt`，最终复验 `.autome/output/m06-final.txt`。
3. 移除 `ARRAY_FIELDS` 中 task.set_after 条目，运行同一测试命令，退出码 1：47 个中 3 失败、44 通过；全部 op 正路径、新操作放行、列表边界均抓到缺陷。日志 `.autome/output/m06-negative-array.txt`。
4. 将 `params.after.length > MAX_AFTER_IDS` 临时改为 false，运行同一测试命令，退出码 1：47 个中 1 失败、46 通过；9 项拒绝断言抓到缺陷。日志 `.autome/output/m06-negative-limit.txt`。
5. 两次注入均通过 finally 还原源码；还原后 47/47 通过，日志 `.autome/output/m06-restored.txt`。`git diff --check` 通过。

本轮用例总数为 47，较本里程碑实测基线 44 增加 3，来自新操作必填校验、位置组合、依赖列表与编号边界三个 test；没有删除旧用例。上一轮 M-05 是 7 个筛选用例和 28 个 CLI 集成用例（其中 4 个重叠），本轮换为网关子集，不将跨套件计数差当作测试增减。未跑项目全量门禁、宿主快检或其它里程碑测试。

## 标待审之前的自审清单

1. **情形表逐行**
   - §8(a)：操作白名单及两处数组白名单已补；全部 op 正路径可达测试通过。
   - §8(b)：set_after 必填列表与 task_id；move 必填 task_id、三种位置互斥与编号格式；create 可选 after，共享同一列表规则，47 个测试通过。
   - §8(c)：create 无 after 仍接受，空列表及 1/8 项接受、9 项拒绝。
   - §8(d)：内核上限由已完成 M-01/M-03 覆盖，本轮网关上限与之相同；未重新执行内核测试。
   - §8(e)：specific 表补两项，中文拒绝文案检查通过。
   - §8(f)：通知器不改，事件随行文档已说明。
   - §9：调度旧文案移除，v6 字段与 store 迁移源码对照，三个事件与 dispatch/scheduler 契约对照；README 三类命令齐全。
2. **失败分支**
   - task_id 缺失/null/空串/数字/布尔/数组/对象拒绝；列表缺失（set_after）、非数组、非编号元素和超限拒绝；位置为零个或多个、编号非法、top 非布尔拒绝，均已执行。
   - 读盘失败、超时、外部服务不可达不适用：本里程碑修改纯请求校验，不进行 I/O。授权机制不改，新增操作的白名单接受及 shape 拒绝由测试覆盖。
3. **全部迁移**
   - 网关自身无状态机，状态迁移不适用。通道检查涵盖创建/修改依赖数组、移动的 before/after 字符串与 top 布尔；位置组合从 000 到 111 全部 8 种均执行，只有 001/010/100 通过。
4. **域边界**
   - 两个列表分别检查 0/1/8/9 项、重复及不重复编号；空串、错误大小写、负数形态、全角数字、换行、非字符串拒绝；T-0/T-01 按正则接受，长数字位置编号接受，存在性留内核校验。
   - top=false 不算位置，和合法 before/after 同时出现接受，单独出现拒绝；task.create 省略 after 接受，task.set_after 省略拒绝。
   - 日期、闰日和跨年不适用，无日期输入。

## 交付边界

仅推进 M-06，状态写 8/30，保留为待审；独立审计和宿主门禁仍待执行。H-01/H-02 人工验收未在本轮执行。未启动下一会话，未推送远端。
