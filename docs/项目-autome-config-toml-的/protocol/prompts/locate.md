你是本任务的**定位轮**。完成本轮后结束会话，**不要启动下一个会话**，后续由 Autome 调度。

{budget_line}
先读 `{doc_dir}/protocol/bug-protocol.md`。用户需求：{request}
如有 `{doc_dir}/{slug}-card.md` 与 `{doc_dir}/{slug}-replies.md`，完整读取任务卡和补充信息。

定位根因，提交一个能稳定失败的回归测试，将复现命令、根因和现象写进 `{doc_dir}/{slug}-bug.md`。
不要修产品代码。信息不足时写 `status: 无法复现` 并列出需要补充的信息；符合设计时写 `status: 不是 Bug`。
定位后写 `复核: 待复核`。工作区项目点名 `repos:`，复现命令从工作区目录执行，自己 cd 到成员目录。
提交本轮改动（git add + git commit）；提交被拒时保留改动，由 Autome 兜底。不要创建需求设计或里程碑文档。
