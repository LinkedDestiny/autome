# M-08 · 实现 #9

结论：待审。范围为 D-R1、D-R5、D-R7 的录入、账本查询、CLI 与网页写网关；送达调度、合并卡点 reopen 与协议改动留给 M-09。本轮未运行全量门禁或宿主快检。

## 实现

- `task.note` 验证输入，按任务递增编号，只追加 `task.note_added` 并广播事件，不触发状态转换或会话启动。已完成/取消拒绝；合并已开始的三节点记录 late 并返回明确 warning。
- `task.get.notes` 从事件按顺序恢复原文、时间和送达会话/角色/轮次/时间；late 备注始终未送达。不迁移数据库。
- CLI 支持直接文本、`--file` 文件与标准输入，读取失败明确报错；status 展示全部备注，run 的两类事件各一行。`--reopen` 参数解析及传递已接入，状态行为按设计在 M-09 实现。
- 网关增加允许操作、散文字段、task_id/text/reopen 校验。8000 上限沿用网页既有 UTF-16 单元计数，内核统一口径；正文保留空格和换行。

## 命令与结果

验收原命令：`cargo test -p automed --lib operator_note_ && cargo test -p autome-cli operator_note_ && npm ci --prefix apps/web && npm run test --prefix apps/web -- test/write-gate.test.ts`。

最终结果：内核 5、CLI 单元 3 + 真实二进制集成 1、网页 51，共 **60 个用例**，全部通过，无跳过。与上一轮 M-07 基线 19 相比 **+41**，因本轮切换到不同验收子集，不能视为全库净增；同口径变更前为网页 47、operator_note 0，共 47，本轮净增 **13**（内核 5、CLI 4、网页 4）。比设计预期 59 多 1 个 CLI 集成用例，没有删除既有用例。

原始输出：`.autome/output/r9-core.txt`、`r9-cli.txt`、`r9-npm.txt`、`r9-web.txt`。首轮编译发现测试误用 Node 引用和 Outcome 字段，已修正；网页初跑 50 过 1 败，原因是既有全部允许操作正路径表缺 task.note 合法参数，补齐后 51 过。

上一轮审计 M-07 通过，无退回缺陷，故不适用“逐字迁入审计缺陷检查”的红绿流程。

## 标待审之前的自审清单

1. **情形表逐行**：录入顺序/原文/不改状态由 `operator_note_is_recorded_in_order_without_changing_state` 检查；终态两行由 `operator_note_rejects_done_and_cancelled_plainly` 检查；Rebase、MergeGate、AwaitMerge 无警告，Merging、MainCheck、Cleanup 有 warning 和 late，由 `operator_note_after_merge_started_is_recorded_with_a_warning` 逐行检查，并验证 late 不被送达事件改成已送达。查询两条备注、一条已送达一条未送达由 task_get 用例检查。CLI 文件/标准输入/直接文本、status 与两类事件、网关四类要求均有对应通过用例。
2. **失败分支**：终态拒绝、空白、超过上限、错误 text/reopen 类型、缺 task_id 均执行且无账本写入；文件不存在、标准输入非法 UTF-8、文本与文件同时指定均执行且报错。拒绝授权、超时、外部服务不可达不适用：本次是本地同步账本写入，无新授权或外部请求路径。
3. **全部迁移**：本轮状态不迁移。用例遍历 15 个需求节点、7 个 Bug 节点，以及 Draft、Queued、Requeued、Paused、Stopped、Failed，逐一验证录入前后状态一致且没有新增会话；Done/Cancelled 拒绝后状态不变。未送达→已送达的读取投影以合成事件验证，late 保持未送达；实际调度属于 M-09。
4. **输入域边界**：空串、空格换行制表符、单字符、8000/8001 中文字符、4000/4001 emoji（UTF-16 为 8000/8002）、600 字散文、非字符串、缺字段和非布尔 reopen 均检查。日期、前导零、闰日不适用：输入没有日期或数字文本解析域。

## 负向对照

- 临时删除 `PROSE_FIELDS['task.note']`，运行网页验收，600 字合法备注被 512 上限拒绝，用例红，退出 1；原始输出 `.autome/output/r9-negative-web.txt`。
- 临时在 task.note 录入时把状态改为 Queued，运行 `cargo test -p automed --lib operator_note_is_recorded_in_order_without_changing_state`，状态不变断言红，退出 101；原始输出 `.autome/output/r9-negative-state.txt`。
- 两处注入均已还原，随后重新执行完整 M-08 验收得到上述 60 个通过；这些对照不计入最终用例总数。
