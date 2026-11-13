# 修复轮不敢格式化回归测试
status: 已定位
fix-round: 0/3
复现命令: cargo test --offline -p automed --test end_to_end --test prompt_rendering --no-fail-fast regression_format_only -- --test-threads=1
根因: 定位提示词缺少格式化检查要求，修复与复核提示词未明确格式化例外；File 的 Git tree 比较及旧 Section 的原文摘要比较拒绝纯格式化。
复核: 待复核

## 现象

任务卡报告 T-64 产品修复通过后，定位轮留下的 Rust 格式差异阻塞合并，修复轮因禁止修改回归测试而不处理，最终耗尽预算。本轮未重跑历史 T-64 或真实模型会话；通过当前代码的实际调度流程与提示词渲染稳定复现对应缺陷。

基线为 `aac9bb5`，不是任务卡中的历史 `d9ad281`。本项目为单 Git 仓库的 Cargo workspace，不是 Autome 多仓库工作区，因此无需 `repos:` 或切换成员目录。命令在本 worktree 根目录执行。

## 复现

执行状态块中的命令。仅运行本轮新增的 7 个测试，离线、不调用真实模型，沿用现有 World 临时仓库和模拟会话驱动真实 scheduler。

- `end_to_end.rs`：创建定位提交及保护快照，再让修复会话仅将单行断言重排为多行并添加行尾逗号，同时修正产品标记；修复复核完成后应到达 `await_merge`。
- 新 File 和旧 Section 摘要快照实际在 Fix 阶段失败，报 `回归测试被修改、删除或重命名`，而不是预期的 `await_merge`。
- 已有测试文件 Blocks、新建 Section 快照均接受格式化。负向对照分别修改四种路径的断言参数，均被保护守卫拒绝。
- `prompt_rendering.rs`：通过真实 `build_prompt` 检查定位提交前格式化检查、quick 命令及无 quick 的 lint 指引；检查两种门禁模式下修复与复核提示词的格式化例外和不可更改的语义边界。

调度测试首跑结果：3 通过、2 失败，退出码 101；提示词测试夹具初次漏传预算，补齐后重新验证，最终失败断言为缺少格式化指引，而非预算或编译错误。最终统一命令复跑：7 个测试中 3 通过、4 失败，退出码 101，与分项结果一致。完整复跑日志见 `.autome/output/regression-format-only-repeat.log`；分项日志见 `.autome/output/regression-format-only.log`、`.autome/output/regression-prompts-final.log`。

本轮只对两个测试文件执行 rustfmt。`cargo fmt --all -- --check` 退出 0，日志 `.autome/output/format-check.log`；`git diff --check` 通过。本地项目配置完整门禁为 `sh scripts/ci-local.sh`、仅合并前运行，未配置 quick；本定位轮未运行全量门禁。

## 根因分析

1. `protocol/prompts/locate.md` 要求提交失败测试，却没有提交前检查新增/修改测试格式以及记录检查结果的要求。`build_prompt` 当前也未向定位会话提供 quick 检查指引。
2. `protocol/prompts/fix.md` 两种门禁分支均绝对禁止修改测试；`protocol/bug-protocol.md` 写“不改回归测试”；`fix-review.md` 也未说明格式化不算弱化。这与 Blocks 的实际允许范围不一致。复核轮自身仍应保持只写任务文档的边界，允许格式化的执行者是修复轮。
3. `scheduler.rs::blocks_kept_in_order` 去除空白与闭括号前逗号后比较，允许重排；`guard_bug_regression` 的 File 路径比较 `ls-tree`，任何字节变化都会改变 blob；旧 Section 快照用 `section_digest` 对原始字节做 SHA-256，格式化同样改变摘要。
4. 对任务卡前提的修正：当前 `regression_protection` 虽返回 Section，但新快照会通过 `added_test_blocks` 保存 `blocks`，实际走 Blocks 比较。原文摘要路径只兼容旧快照。本轮特意构造真实定位内容对应的旧摘要快照验证该分支，未把新 Section 误报为失败。

后续修复需统一文字与三种保护方式的格式化口径，保留对断言、输入、用例名、跳过标记等改动的拒绝。旧摘要无法单独恢复原文，兼容处理需考虑快照已有的 head/path；不应通过无条件接受绕过保护。现有 Blocks 的字符串压缩并不是语言语义分析，不能把字面量内部的空白变化当成无害格式化。

协议修复按任务卡要求发布独立 `## protocol/vN`、`clarify` 记录并同步金样；已固定旧协议的运行任务不迁移。本轮未改产品代码、协议正文或金样，也未把历史真实会话的行为作为本轮实测结果。

## 修复说明

待修复轮填写。

## 复核意见

待独立复核。
