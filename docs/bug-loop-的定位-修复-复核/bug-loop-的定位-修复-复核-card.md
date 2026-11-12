# Bug Loop 的定位 / 修复 / 复核 prompt 列出任务附件与 `--doc` 参考
## 背景
计划 §3.1：附件和参考文档是任务卡的一部分，是定位轮的输入。内核已经把附件复制进任务目录并提交（`scheduler.rs:2822-2833`），但只有需求的整理轮 prompt 把附件和 `doc_refs` 渲染进去（`launcher.rs:685-728`，`{inputs}` 只出现在 `protocol/prompts/intake.md`）；`bug_prompt`（`launcher.rs:492-540`）和 `locate.md`/`fix.md`/`fix-review.md` 都没有。带截图或日志的 Bug 卡，定位轮不知道有附件；`--doc` 指定的参考文档同样被忽略。
## 要做什么
1. `bug_prompt` 复用整理轮的 `inputs` 渲染（附件相对路径列表 + 参考文档列表），三个 Bug 步骤的 prompt 模板加 `{inputs}` 占位；无附件、无参考时不输出空段落。
2. 协议版本号按规则递增（`protocol/`），`protocol/CHANGELOG.md` 记一条。
3. 测试：`launcher.rs` 单测（有/无附件两种）、`prompt_rendering` 金样更新、e2e：带附件与 `--doc` 创建 Bug 任务后，定位会话的 prompt 文件含附件路径与文档路径。
## 不做什么
不改需求 Loop 的 prompt；不改附件上传与存储。
## 验收标准
见测试；`cargo test --workspace` 全绿；`plugins/autome` 的 eval 不受影响。
## 参考
`crates/automed/src/launcher.rs`、`protocol/prompts/{locate,fix,fix-review}.md`、计划 §3.1、§4.2。
