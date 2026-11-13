# M-05 · 实现轮 6/25

## 范围与修订

仅推进 M-05 / G8，修改 README Host gate 配置与 Merge queue 说明、CLI 撤销示例，以及技术设计 §3.3、§5.3、§5.6、§8.1、§14。未修改运行代码、测试、配置或协议。旧的门禁沿用说明同步加上“不落后”条件，避免与新段落矛盾。

对照当前两个 crate 的 merge_queue.rs、dispatch::task_merge、scheduler 的锁调用位置核实授权优先级、状态限制、队列与投影；按任务设计 D1–D8 核对全部文档要求。只读 `git log main --grep='T-53\|T-51'`：T-51 已合入，T-53 未命中，不 fetch、不操作其它 worktree。上一轮 M-04 审计通过，无须迁移的缺陷复现检查。

## 命令与结果

验收命令（逐字执行）：

```sh
grep -q '^auto = ' README.md && grep -q 'Merge queue' README.md && grep -q 'merge cart --cancel' README.md && grep -q '合并队列' docs/development/technical-design.md && grep -q 'merge.locked' docs/development/technical-design.md && grep -q 'MergeGate 出队冲突' docs/development/technical-design.md && echo docs-ok
```

输出 `docs-ok`，退出码 0。验收总数 **6 项文档检查**，较上一轮 M-04 验收基线 6 项差 **0**，但口径由 Rust 行为用例切换为文档断言，不能视为重跑了原来 6 个用例。本轮没有新增或删除 Rust 测试。若与上一轮含相关回归的去重总数 35 比，本轮 6 项差 **-29**，原因是纯文档里程碑未执行 CLI 渲染等行为回归，不是丢失测试。负向对照不重复计入验收总数。`git diff --check` 退出 0。

负向对照仅操作 `.autome/output/` 临时副本：`sed '/Merge queue/d' README.md` 后对应 grep 退出 1；`sed '/merge.locked/d' docs/development/technical-design.md` 后对应 grep 退出 1。两项符合预期，原文未删改。首次副本删除表达式误写成不含句点的粗体标题，未删中，检查返回失败；改成上述精确关键词删除后两项均成功检出缺失。副本为 m05-readme-negative.md、m05-technical-negative.md。

未运行全量门禁或宿主快检组合；按本轮指令由宿主运行。未执行人工验收 H-01/H-02，本轮文档检查不替代人工验收或前轮行为测试。

## 标待审前自审清单

1. **情形表逐行**：按“文档与协议”三行逐项核对。README：auto 缺省、出队对齐、锁起止、等锁不堵队列、授权/撤销、无人最终审查的取舍、无 gate 警告、CLI 示例均已写入；技术设计：配置、需求转换、Bug 出队冲突、D1–D4/D8 队列、RPC 参数/结果、merge 投影均已写入；协议：不修改，沿用 RebaseConflict。六项 grep 全部通过。
2. **失败分支**：M-05 点名的两条文档缺失分支已各用临时副本负向检查，均返回预期 1。运行时拒绝授权/撤销、准备失败、门禁失败、冲突、外部 main 前进、Git 读取失败降级已逐项核对文档描述；本轮无运行代码变化，未重跑这些行为分支。
3. **全部迁移**：逐行核对 D3 的 13 行：门禁/准备失败、出队冲突、撤销、暂停/停止/取消、作业消失/head 变化、通过有授权、通过撤销、移除 gate、合并 Ok/Blocked/Behind/Failed、重启，§8.1 均有相应释放/保留/恢复说明；§5.3/5.6 描述需求与 Bug 转移，含纳入表态例外。本轮是文档变更，运行全部状态机迁移不适用，未声称已重新执行。
4. **域边界**：逐项核对 auto 缺省 false/true/仅布尔、cancel 缺省 false/仅布尔、无授权/Failed/终结/已开始合并、无 gate 的需求/Bug、stale 仅 AwaitMerge/零落后/null/工作区求和/读失败。说明与源码和设计一致。本轮没有新输入域，数值最大值、日期、空串等运行边界测试不适用。
