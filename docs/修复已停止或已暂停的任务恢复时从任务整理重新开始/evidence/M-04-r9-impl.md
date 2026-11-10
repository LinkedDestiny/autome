# M-04 实现证据（r9）

结论：验收通过，提交待审；本轮只推进 M-04。

## 修改与范围

`crates/autome-cli/src/render.rs` 根据 queued 的可选 resume 字符串显示「排队（会从 {节点} 继续）」，复用 node_label；没有 resume 的新建排队仍显示「排队」。CLI 各状态显示入口共用 state_line，task.updated 事件同样使用它。未改 idle 判断（属于 M-05）。上一轮审计为通过，没有审计退回检查需要搬入。

## 命令与结果

- 红：先只加入两个项目内渲染测试，执行 `cargo test -p autome-cli`。主程序套件 10 个：9 通过、1 失败，`requeued_states_show_the_original_node` 得到「排队」而期望节点提示；cargo 报测试失败，后续集成套件未执行。原始输出 `.autome/output/m04-r9-red.txt`。这同时证明去掉节点提示的负向对照会被新增测试拦住。
- 绿：修改 queued 渲染后执行 `cargo test -p autome-cli`，10 个主程序测试 + 4 个集成测试全部通过，0 失败、0 忽略。原始输出 `.autome/output/m04-r9-green.txt`。
- `cargo fmt --all -- --check` 通过，无输出，记录 `.autome/output/m04-r9-fmt.txt`；`git diff --check` 通过。
- 用例总数 14，相比上一轮 CLI 基线 12（8 + 4）增加 2，来自恢复节点显示与新建排队保持原文字两个测试。13 节点循环是一个测试，不重复计数。上一轮工作区基线为 912；本轮未跑工作区全量，不能把推算的 914 当作实跑结果。工作区全量与 clippy 留给 M-05 合同，宿主门禁由 Autome 执行。

## 标待审之前的自审清单

1. **情形表逐行**：恢复排队有 resume → 显示原节点，通过 `requeued_states_show_the_original_node`；新建排队无 resume → 显示「排队」，通过 `new_queued_states_keep_the_plain_label`。task.updated 事件中同样逐节点核对完整文字，全部通过。
2. **失败分支**：不适用。M-04 为 JSON 状态的纯文本渲染，设计未指定授权拒绝、读盘、超时或外部服务失败分支；缺失与 null 的 resume 已检查，保持普通排队显示。
3. **全部迁移**：不适用状态迁移，本轮不改变转换或调度。显示域已逐个检查 13 个可恢复节点：intake、design、review、adjudicate、implement、gate、audit、retro、rebase、merge_gate、merging、main_check、cleanup；每个节点同时验证 state_line 与 task.updated 两个显示通道，均通过。暂停和停止恢复共用同一 queued + resume JSON 形状。
4. **域边界**：有效恢复节点为上述有限 13 元域，已全部枚举；无 resume 与显式 null 两个可选字段边界均通过。空串、数值最大值、闰日等不是本里程碑合法 Node 输入，不适用。未扩大生产输入校验范围。
