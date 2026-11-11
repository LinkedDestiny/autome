# M-04 实现证据 · 实现 #5（5/30）

范围：只推进 M-04。起点 `972de55`。上一轮审计 M-03 通过，没有退回缺陷或需要逐字移植的审计检查。

## 实现说明

- `task.reply` 对需求与旧 Bug 委托原 reject 路径；新 Bug 校验非空、状态机校验成功后记录 `task.replied` 原文，再启动定位。拒绝不会留下回复事件。
- 定位前按事件顺序生成并提交 `-replies.md`；使用任务冻结的 doc_dir，正文不参与模板替换。写盘失败显式报错，已接受的信息仍保留在账本。
- Bug 的 JSON 添加 `fix_round`，需求字段集合不变；Bug 手动复盘被拒绝。CLI 映射、七个节点、七种事件、等待操作和预算失败提示已实现。
- 工作区首次定位检出全部成员，Bug 文档 `repos` 必须非空且有效；定位后只移除未点名且干净、相对基线没有新增提交的成员，文档成员保留。后续集合继续取声明与已有检出的并集。
- 修复预算复用既有领域转换，没有另写计数规则；本轮增加真实调度器的配置 2 → 用完 → 追加 1 → 第 3 轮通过证明。
- 既有测试没有删除或改动；§11 第 5 条的过渡输入调整已在实现 #4 完成，本轮补齐对应 Bug CLI 用例。

## 标待审前的自审清单

四项均已逐项执行。下面的最终验收覆盖全部列出情形；所有判据通过。

1. **情形表逐行**：§5 的 feature/旧 Bug reject、Bug 空文本拒绝、错误节点拒绝、两种等待节点回复、回复事件原文与文件提交；§7 的单仓库/工作区全部成员/有效 repos/空 repos/未知成员/文档成员保留；§9 的仅 Bug fix_round、七节点、七事件、两个等待提示和 FixBudget 提示。逐项对应本轮 13 个新用例，最终专项与全量均通过；工作区另跑未点名 b 已有提交的保留对照。
2. **失败分支**：空串与全空白、错误节点、等待态 pause/stop/extend 拒绝且无回复事件；回复目标被目录占用导致写盘失败；工作区缺 repos 或非法成员进入 Protocol；Bug retro 拒绝；预算追加 0 和 u32::MAX 拒绝且预算不变。授权拒绝、网络服务不可达不适用：本轮没有新增授权或外部服务路径。超时/复现不可执行沿用既有领域和 M-03 检查，本轮全量复验，不宣称真实网络或模型验收。
3. **全部迁移**：locate→await_info、locate→not_a_bug；两种等待态 reply→locate；not_a_bug→cancelled/归档；同正文创建 feature 草稿；repro_check 首次未复现→locate、第二次→await_info，reply 后计数重新开始；fix_gate 失败→fix，第二轮耗尽→FixBudget，extend→第 3 轮 fix→fix_review→await_merge。需求和旧 Bug 在批准设计等待态 reply/reject 的结果、状态与事件种类一致；需求 Draft 下两者错误一致。剩余 Bug/需求状态组合由领域全量已有测试复验。
4. **域边界**：空串、空白、多字节中文、双空格、换行、尾换行、模板形状 `{budget_line}`/`{request}` 原样保留；预算 0、上限后再追加 1、u32::MAX 溢出；repos 空集合、单成员和未知成员。前导零/日期/闰日不适用：本轮 RPC 无日期或数字字符串输入域。

## 验证记录

| 命令 | 用例总数 / 上轮基线 / 差值 | 结果与日志 |
|---|---|---|
| `cargo test -p automed --test end_to_end bug_loop` | 14 / 8 / +6 | 全通过；`.autome/output/m04-final-e2e.txt` |
| `cargo test -p automed --lib dispatch::tests::bug_loop` | 3 / 0 / +3 | 全通过；`.autome/output/m04-final-dispatch.txt` |
| `cargo test -p autome-cli bug_loop` | 4 / 0 / +4（单元 2、集成 2） | 全通过；`.autome/output/m04-final-cli-bug.txt` |
| `cargo test -p autome-cli` | 29 / 25 / +4（19 单元、10 集成） | 全通过；`.autome/output/m04-final-cli-all.txt` |
| `cargo test --workspace` | 1071 / 1058 / +13 | 全通过，0 失败、0 忽略；`.autome/output/m04-workspace.txt` |
| `cargo clippy --all-targets -- -D warnings` | 非用例计数 | 退出 0；`.autome/output/m04-clippy.txt` |
| `cargo fmt --all -- --check`、`git diff --check` | 非用例计数 | 均退出 0；格式日志 `.autome/output/m04-format-check.txt` |

全量拆分：CLI 单元 19、CLI 集成 10、domain 345、automed 单元 601、e2e 73、prompt 16、real_cli 7，共 1071。相对上一实现轮证据的完整基线 1058，增加 13，恰好来自本轮 6 个 e2e、3 个 dispatch、2 个 CLI 集成、2 个 CLI 单元；既有用例一个未删除。专项与全量重叠，不重复累计。real_cli 的条件性检查不等于真实模型验收。

### 设计负向对照

每次只注入一个缺陷，执行下列命令，随后恢复源码；最终验收在全部恢复后的源码上通过。五次均为真实断言失败（各 1 个用例失败、退出 101），不是编译失败。

| 注入 | 命令 | 红证据 |
|---|---|---|
| 关闭工作区 repos 守卫 | `cargo test -p automed --test end_to_end bug_loop_a_workspace_bug_checks_out_every_member` | `.autome/output/m04-negative-repos.txt` |
| 需求回复错误使用 Trigger::Reply | `cargo test -p automed --lib dispatch::tests::bug_loop_reply_on_a_feature_task` | `.autome/output/m04-negative-feature-reply.txt` |
| 不生成补充信息文件 | `cargo test -p automed --test end_to_end bug_loop_cannot_reproduce_waits` | `.autome/output/m04-negative-replies-file.txt` |
| ExtendBudget 委托需求转换表 | `cargo test -p automed --test end_to_end bug_loop_fix_rounds_run_out` | `.autome/output/m04-negative-extend.txt` |
| 对所有任务输出 fix_round | `cargo test -p automed --test end_to_end bug_loop_task_get_reports` | `.autome/output/m04-negative-json.txt` |

这些检查已经位于项目 Rust 测试体系。上轮审计没有退回缺陷，因此没有需要从审计临时文件逐字移植的检查。

### 自审判据落点

- 情形 1（补充信息）：完整文件字节与 `git show HEAD:<path>` 相等，后续两次 Locate 都点名 replies 文件；空串/空白/非法控制不产生 `task.replied`。文件写失败另起隔离项目检查，状态失败且账本仍保留原文。
- 情形 2（不是 Bug）：暂停/停止/追加均拒绝；reply 再定位仍能等待；同正文 feature 创建成功；取消后归档。
- 情形 3（连续未复现）：第一次两次未复现等待，reply 后允许再跑两次，累计 4 条 miss，证明按回复重置周期。
- 情形 4（预算）：真实配置上限 2、两轮失败进入 FixBudget；0 与 u32::MAX 不改变预算；追加 1 后第 3 轮通过并进入 await_merge。
- 情形 5（JSON）：feature 的 15 个键与基线字面量相等；Bug 仅额外增加 fix_round。实际修复任务另断言 fix_round=3、budget_n=3。
- 情形 6（工作区）：a/b/docs 初始可见；有效单成员进入 await_merge，复现从工作区 cwd 通过 `cd a` 执行；空/未知 repos 均 Protocol；未点名无提交 b 移除，有提交 b 保留。复核和门禁在实际成员检出上运行。
- dispatch 与 CLI：需求/旧 Bug 回复路径、写方法分类、Bug retro 拒绝、真实 CLI 原文、真实 status 下一步，七节点/七事件/失败提示与帮助文本均通过。

开发中曾修正新测试夹具：配置文件尚不存在改用已有 `configure`；工作区既有 `settle` 未等待 Bug gate，新用例使用有截止时间的真实状态轮询，未改既有 helper；parity 夹具补初始提交避免比较不同临时路径的 Git 错误。这些不构成产品退回，最终所有新用例均在真实调度/门禁链路验证。

## 边界与交接

仅修改当前 worktree 的 crates 与本任务记录；无依赖、schema、协议模板或 apps 改动，无外部服务调用、真实用户账本、推送或下一会话。M-05/M-06 保持开放。桌面/Web 与真实模型人工验收本轮未运行；`sh scripts/ci-local.sh` 由 Autome 按任务 C11 在实现轮后执行，Rust 全量通过不冒充完整宿主门禁。

本轮提交使用仓库既有 `NO_CI=1`，避免 post-commit 自行重复启动后台宿主 CI；Autome 的下一节点和独立宿主验收保持原调度。

