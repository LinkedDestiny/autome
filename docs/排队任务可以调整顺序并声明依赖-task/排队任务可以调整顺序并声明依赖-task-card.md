# 排队任务可以调整顺序并声明依赖

## 背景

排队任务现在严格按进入排队的先后出队（`store.queued_tasks` 按 `queued_seq, created_at` 排序，`fill_slots` 从头取）。没有任何办法把一个排队任务提前或推后，也没有办法说"这个任务要等另一个任务合并之后再开始"。

两个真实场景：

- `docs/plans/2026-11-10-web-dashboard-via-loop.md` §7.3 明确要求 W6 在 W5 之后、W7 在 K2 之后、W10 在 W12 之前。现在只能靠人工晚一点创建，或者创建成待开始再手工 `task start`，等待的人要一直盯着。
- 2026-11-12 项目里排了 9 个任务（T-44 到 T-52），其中 T-51（approve/merge 绕过并行上限）和 T-47（门禁排队）是其它任务的减速带，应该先做；T-52 依赖 T-41 的改动。现在它们只能按编号顺序等。

一个靠 Loop 自己推进项目的产品，队列的先后和依赖是它的核心控制面，不应该靠人工时机。

## 要做什么

1. **依赖**：任务可以声明"在哪些任务之后"。
   - `task.create` 新增 `after: ["T-41"]`（同项目的任务 id 列表，可空）；命令行 `autome task new … --after T-41[,T-47]`。
   - 新 RPC `task.set_after { task, after: [...] }`，可以改、可以清空；命令行 `autome after <任务> T-41,T-47` 和 `autome after <任务> --none`。只允许对待开始、排队、等你决定（设计卡点之前）的任务修改；已经开始实现轮的任务拒绝并说明原因。
   - 依赖只能指向同项目的任务；指向自己、形成环、指向不存在的任务，都在创建或修改时拒绝。
   - store 迁移一个版本，旧任务没有依赖。
2. **出队规则**：`fill_slots` 跳过任何一个依赖还没有到已完成的排队任务，但不改它的 `queued_seq`，后面的任务可以先走。依赖到已完成（Done）就自然轮到它。
   - 依赖被取消、失败或归档：被依赖的任务**不**自动开始，也不失败；它留在排队，`ls` / `status` / `dashboard.get` 写明"等 T-41（已取消）"，并写一条事件 `task.blocked { task, on, reason }`，通知器能据此提醒用户。用户用 `autome after … --none` 解除后照常出队。
3. **顺序**：新 RPC `task.move { task, before?: id, after?: id, top?: bool }`，只对排队和待开始任务有效（待开始的任务 `task start` 时按它新的位置插入）。命令行 `autome queue move <任务> --before T-44 | --after T-46 | --top`。写一条事件 `task.moved`。
4. **可见**：`task.get`、`project.get`、`dashboard.get` 的任务都带 `after`（声明的依赖）和 `blocked_by`（其中还没完成的那些，以及每个的当前状态）。`autome ls` 的排队段按真正的出队顺序列出，每行末尾加"← 等 T-41"；`autome status <任务>` 多一行"依赖"。所有输出 `--json` 一致。
5. 网关 `apps/web/server/gates/write-gate.ts` 的两份白名单加上 `task.set_after`、`task.move`（照 `task.extend_budget` 的输入校验写上限：id 列表最多 8 个）。

## 不做什么

- 面板上的拖动排序和依赖编辑界面（留给 W6/W7 的 Backlog，本任务只做内核、CLI 和网关放行）。
- 跨项目依赖。
- 根据任务目录重叠自动推断依赖。
- 依赖完成时自动合并或自动批准，什么都不自动化，只控制"什么时候开始"。

## 验收标准

1. e2e：`parallel = 2`，创建 A，再创建 B `--after A`，再创建 C。驱动跑到稳定后 A 和 C 在跑、B 仍在排队；把 A 一路推到已完成后 B 自动开始。全程没有手工步骤。
2. e2e：A 被取消后 B 留在排队并带 `blocked_by = [A(已取消)]`，事件里有 `task.blocked`；`task.set_after` 清空后 B 在下一个 tick 开始。
3. e2e：三个排队任务，`task.move` 把第三个提到最前，下一个 tick 先启动它。
4. 单测：自引用、环、跨项目、不存在的 id 都被拒绝，拒绝信息是中文的一句话。
5. 现有的需求和 Bug e2e 不做修改仍然通过；store 迁移测试覆盖新版本。
6. CLI 集成测试：`task new --after`、`after … --none`、`queue move --top`、`ls --json` 里的 `after`/`blocked_by`。
7. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿。

## 参考

- `crates/automed/src/store.rs` `queued_tasks`（排序）与 `queued_seq` 的赋值（`UPDATE tasks SET queued_seq = …`）。
- `crates/automed/src/scheduler.rs` `fill_slots` / `fill_slots_reporting_errors`。
- `crates/automed/src/dispatch.rs` `task.extend_budget`、`task.start` 的写法（写方法、参数校验、事件）。
- `crates/autome-cli/src/` 现有子命令的表驱动解析；`crates/autome-cli/tests/cli.rs`。
- `apps/web/server/gates/write-gate.ts`、`read-gate.ts`。
- `docs/plans/2026-11-10-web-dashboard-via-loop.md` §7.3。
