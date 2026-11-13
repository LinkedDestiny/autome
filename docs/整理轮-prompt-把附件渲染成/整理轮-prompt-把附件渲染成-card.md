## 现象
需求任务带附件时，整理轮 prompt 里列出的附件路径指不到文件。`intake_prompt`（`crates/automed/src/launcher.rs` 约 685-704 行）把每个附件渲染成 `- {doc_dir}/attachments/{a}`，其中 `a` 是 `task.attachments` 的存储原值；而两个真实入口存的都是**源文件绝对路径**（CLI `--attach` 经 `canonicalize`，`main.rs` 410-418；Web 网关解析上传 id 成 `<home>/web/uploads/...`，`gateway.ts` 44-54），`write_task_inputs`（`scheduler.rs` 约 2878-2897）复制副本时只取 `file_name()`。结果 `/Users/x/crash.log` 在 prompt 里变成 `docs/<slug>/attachments//Users/x/crash.log`。现有单测 `the_intake_prompt_lists_attachments_and_doc_refs_when_present`（`launcher.rs:1816`）用裸文件名 `promo.csv`，替身与调用方对齐而没有与真实入口对齐，所以一直是绿的。T-41 设计轮实测发现（其设计文档「背景与现状」第 2 条），因超出其范围未修。
## 复现步骤
1. `autome task new "x" --attach /tmp/crash.log -p <项目>`，启动。
2. 看整理轮的 prompt（`.autome/output/sessions/<task>/<ses>.prompt`）：附件行是 `- docs/<slug>/attachments//tmp/crash.log`。
3. worktree 里实际文件是 `docs/<slug>/attachments/crash.log`。
## 期望行为
- 整理轮 prompt 列出的附件路径是副本的真实路径 `{doc_dir}/attachments/<文件名>`（与 `write_task_inputs` 同一规则；T-41 正在给 Bug 侧做同样的映射，复用其 `render_inputs` + 显示名映射，不要再写一份）。
- 单测改为用绝对路径（含空格）的替身，与真实入口对齐；T-41 新增的 `the_intake_prompt_keeps_rendering_stored_attachment_values_verbatim`（固定缺陷行为的那条）在本任务里改成断言正确路径。
- `intake` 金样若受影响则更新；e2e：带 `--attach` 的需求任务，整理轮 prompt 里的附件路径在 worktree 中存在。
- 与 T-41 的顺序：T-41 先合并（本任务 rebase 到它之上）。
## 环境
main，`crates/automed/src/launcher.rs`、`crates/automed/src/scheduler.rs`、`crates/automed/tests/prompt_rendering.rs`。
