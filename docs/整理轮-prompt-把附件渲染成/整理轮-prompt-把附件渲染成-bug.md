# 整理轮附件路径定位

status: 已定位
fix-round: 0/3
复现命令: cd crates/automed && cargo test --offline -p automed --lib the_intake_prompt_
根因: intake_prompt 将存储的源文件绝对路径直接传给按附件文件名拼接的 render_inputs，与 write_task_inputs 仅取 file_name 的复制规则不一致。
复核: 待复核
repos: autome-v2

## 现象

整理轮把源文件 `/tmp/autome source/promo data.csv` 渲染为 `docs/checkout-flow/attachments//tmp/autome source/promo data.csv`，而附件副本应为 `docs/checkout-flow/attachments/promo data.csv`。Web 上传绝对路径也有相同问题。

基线提交为 `2fd793e`，已存在 T-41 的 `render_inputs`、Bug 附件显示名映射及固定整理轮缺陷行为的测试。本轮未切换分支或 rebase。

## 复现

从本 worktree 根目录执行状态块中的命令；命令自行进入 Cargo 工作区成员 `crates/automed`。使用本机缓存离线运行，不依赖服务、网络、真实上传或时间等待。

修改 `crates/automed/src/launcher.rs` 的测试区：

- `the_intake_prompt_lists_attachments_and_doc_refs_when_present` 使用含空格的源文件绝对路径，断言完整附件行及参考文档仍存在。
- 将固定缺陷的 `the_intake_prompt_keeps_rendering_stored_attachment_values_verbatim` 替换为 `the_intake_prompt_resolves_absolute_attachments_to_task_copies`。输入分别模拟 CLI 和 Web 网关存储的绝对路径，精确比较附件行，覆盖无任务卡及有任务卡分支。当前先在无任务卡分支失败，有任务卡断言尚未执行到。

两次运行均退出 101，结果为 3 passed、2 failed；失败来自路径断言，不是编译、超时或命令缺失。核心差异：

```text
实际: - docs/checkout-flow/attachments//tmp/autome source/promo data.csv
预期: - docs/checkout-flow/attachments/promo data.csv
实际: - docs/checkout-flow/attachments//tmp/autome-home/web/uploads/upload-123/screen shot.png
预期: - docs/checkout-flow/attachments/screen shot.png
```

原始日志位于 `.autome/output/intake-attachment-regression.log` 与 `.autome/output/intake-attachment-regression-repeat.log`，不纳入提交。最初 `--bin automed` 运行了 0 项测试，不作为复现证据；有效目标为 `--lib`。

本轮验证的是真实 prompt 构建函数的路径契约；未运行真实 CLI 建任务、worktree 副本存在性 e2e 或完整项目门禁。现有 intake 金样的附件为空，本轮无需更新。

## 根因分析

1. CLI `crates/autome-cli/src/main.rs:421` 对 `--attach` 执行 `canonicalize`，将绝对路径写入附件字段；Web `apps/web/server/gateway.ts:44` 将上传 ID 解析为路径并强制检查 `isAbsolute`。
2. `crates/automed/src/scheduler.rs:3222` 的 `write_task_inputs` 通过 `Path::file_name()` 取文件名，复制到任务目录的 `attachments` 下。
3. `crates/automed/src/launcher.rs:861` 的 `render_inputs` 接收附件显示名并拼接任务目录；但 `intake_prompt` 在第 901 行直接传入存储原值，因此把绝对路径嵌入相对路径中。空格不是根因，绝对路径的目录部分才是。
4. 同文件第 604 行已有文件名映射 `bug_attachment_name`，Bug 轮第 669 行使用映射结果，而整理轮漏用。旧裸文件名替身掩盖了入口契约差异；另一个旧测试明确断言错误路径，进一步固定了缺陷。

修复轮应按任务卡要求复用现有映射和 `render_inputs`，与附件复制规则保持一致。本轮仅修改测试，未修改产品代码。

## 修复说明

待修复轮填写。

## 复核意见

待独立复核。
