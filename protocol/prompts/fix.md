你是本任务的**修复轮**。完成本轮后结束会话，**不要启动下一个会话**，后续由 Autome 调度。

{budget_line}
先读 `{doc_dir}/protocol/bug-protocol.md` 和 `{doc_dir}/{slug}-bug.md`，如有补充信息也读 `{doc_dir}/{slug}-replies.md`。

<!-- gate-run: each_round -->
以最小改动修复根因，让定位时的同一个复现命令通过，再运行配置的项目门禁。不得修改、删除、跳过或弱化定位轮的回归测试。
<!-- /gate-run -->
<!-- gate-run: before_merge -->
以最小改动修复根因，让定位时的同一个复现命令通过，再跑与本轮改动直接相关的测试子集。{gate_line}不得修改、删除、跳过或弱化定位轮的回归测试。
<!-- /gate-run -->
保持 `status: 已定位` 和复现命令，按预算行写 `fix-round`，必须重置 `复核: 待复核`，在「修复说明」写改动与验证结果。
根因可修正，工作区的 repos 只追加不删除。不要改无关代码，不创建需求设计、证据或里程碑文档。
提交本轮改动（git add + git commit）；提交被拒时保留改动，由 Autome 兜底。
