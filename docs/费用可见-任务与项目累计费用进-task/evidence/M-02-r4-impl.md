# M-02 · 实现第 4 轮

## 实现与范围

- `store.sessions_in_project` 使用 SESSION_SELECT_BASE + 项目任务子查询，每项目一次查询，不 JOIN、不逐任务取费用。
- dispatch 按任务分组后复用 `CostSummary::of`；项目 get/list 返回 finished/unfinished 总额和缺失费用计数；所有 task_json 调用方传入费用，看板 waiting/running 同步获得字段。task.get 复用已有 sessions。
- Done、Cancelled 归 finished，其余归 unfinished；空集合返回数值 0。既有字段、断言、read 白名单、调度与桌面渲染代码未改。
- 上轮审计 M-01 通过，无退回检查需要搬迁。本轮没有修改既有测试断言。

## 命令与结果

- `cargo test -p automed --lib cost_rollup_`：总数 3，3 通过、0 失败、0 跳过；相对本过滤器基线 0 增加 3，分别是六状态项目汇总、项目隔离、看板卡片测试。
- `cargo test -p automed --lib`：总数 651，651 通过、0 失败、0 跳过；上轮 `.autome/output/r3-ci.txt` 的同套库测试为 648，差 +3，全部来自上述新增用例。
- `cargo fmt --all --check`、`cargo clippy -p automed --lib --tests -- -D warnings`、`git diff --check`：通过。
- 原始输出：`.autome/output/m02-tests.txt`、`m02-lib-final.txt`、`m02-fmt-check.txt`、`m02-clippy.txt`。
- 首次编译测试夹具误用了不存在的 Node::Plan，改为真实 Node::Design 后通过；不涉及产品缺陷。
- 本轮未运行 workspace 全量、桌面/Web 测试和 ci-local.sh；这些仍由宿主门禁及 M-05 覆盖，不把上轮通过当成本轮证明。

## 标待审之前的自审清单

1. **情形表逐行**：已结束且有费用计入原始和；已结束且无费用计 0、缺失数 +1；运行中（有费用和无费用两种夹具）均排除。项目六种状态的金额依次 1–6，finished=3、unfinished=18、missing=6，每张任务卡逐一断言。两个有费用项目与一个空项目互不串数，get/list 对齐；waiting/running 卡片分别 1.25/2.5、missing 各 1。以上均由三个验收用例执行通过。
2. **失败分支**：本里程碑未点名授权、读盘、超时、外部不可达的新失败分支，不适用（只扩展现有只读接口；SQL 错误按原 Result 链传播）。空输入适用：空项目总额双 0、缺失 0；新建无会话任务卡费用 0，已执行通过。
3. **全部迁移**：不适用新增状态机迁移，本轮没有状态转换逻辑。仍检查只读聚合对状态变化的响应：Active→Cancelled 后 finished 3→9、unfinished 18→12，总额不变。project.get、project.list、dashboard waiting/running 四个读取通道全部执行通过。
4. **输入域边界**：空项目、无会话任务、单个有费用会话、缺失费用、运行中会话、小数金额均执行通过。没有新增日期、字符串或最大金额输入域；前导零、闰日、跨年、最大值不适用，费用沿用既有 f64 原始和。

## 负向对照与复原

每次仅临时注入一个缺陷，运行 `cargo test -p automed --lib cost_rollup_`，之后恢复源码：

- finished 仅判断 Done：退出 101，3 中 1 失败，六状态用例拦住漏算 Cancelled。
- 项目查询移除项目过滤（保留绑定参数）：退出 101，3 中 1 失败。测试直接核对 sessions_in_project 返回行数，防止下游按 task_id 筛选掩盖取数越界。
- task_json 固定费用 0：退出 101，3 中 2 失败，项目卡片与看板卡片均拦住。
- 日志分别为 `.autome/output/m02-negative-terminal.txt`、`m02-negative-isolation.txt`、`m02-negative-card.txt`。三项均已复原，最终全库及验收再次通过。

结论：M-02 待审，无阻塞；只推进此里程碑。后续由审计独立复验，不自行启动会话。
