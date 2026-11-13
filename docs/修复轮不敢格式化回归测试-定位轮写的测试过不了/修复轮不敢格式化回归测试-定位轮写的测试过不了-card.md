# 修复轮不敢格式化回归测试：定位轮写的测试过不了 `cargo fmt --check`，修复轮以「不得修改回归测试」为由不处理，任务耗尽轮次

## 现象

T-64（Bug）的产品修复在第 2 轮就通过了独立复核，但合并前门禁因 `cargo fmt --all -- --check` 失败：定位轮在 `crates/automed/tests/end_to_end.rs` 新增的回归测试里有一行过长的单行 `if/else`。第 3 轮修复没有处理它，复核打回：

> 阻塞项：`cargo fmt --all -- --check` 本轮复验仍退出 1，唯一差异为 `end_to_end.rs:2631` 的单行 `if/else` 需展开为多行。这是纯格式差异，不改变测试语义，修复轮因「不得修改回归测试」而未处理……需要用户追加轮次或明确放行。

任务以「修复轮次用完（上限 3）」失败。操作者手工在 worktree 里跑了一次 `cargo fmt` 并追加一轮才继续。

内核其实允许这种改动：`crates/automed/src/scheduler.rs` 的 `blocks_kept_in_order` 比较受保护的测试块时忽略空白与行尾逗号，注释写着「the gate's formatter may rewrap the added tests, and that is not an edit」。是 prompt 与协议的措辞（`protocol/prompts/fix.md`「不得修改、删除、跳过或弱化定位轮的回归测试」、`protocol/bug-protocol.md`「不改回归测试」）让会话把格式化也当成禁区；定位轮的 prompt 也没要求新增的测试先过格式化。

## 复现步骤

1. 起一个 Bug 任务，让定位轮在已有的 Rust 测试文件里新增一个不符合 rustfmt 的回归测试（例如一行超过 100 列的 `let v = if a { "x" } else { "y" };`），复现命令能正确失败；
2. 修复轮修好产品代码，复现命令通过；
3. 合并前门禁（含 `cargo fmt --check`）失败并退回修复轮；看修复轮的 prompt 与它的处理。

## 期望行为

- **定位轮自己交出合规的测试**：`protocol/prompts/locate.md` 要求定位轮在提交前对自己新增 / 修改的测试文件跑项目的格式化检查（项目 `[gate] quick` 有配置时，prompt 给出该命令；没有时写「项目的格式化 / lint 检查」），并在 Bug 文档「复现」一节记录结果。
- **修复轮知道格式化不算修改**：`protocol/prompts/fix.md`、`protocol/prompts/fix-review.md` 与 `protocol/bug-protocol.md` 写明「只改变空白、换行、行尾逗号的格式化（例如 rustfmt、prettier 的自动重排）不算修改回归测试，门禁要求时应当做；断言、输入、用例名、跳过标记一律不得改」，与内核 `blocks_kept_in_order` 的实际口径一致。复核轮据此不把纯格式化判为弱化。
- 内核的保护口径与文字一致：对三种保护方式（`File` / `Section` / `Blocks`）逐一确认纯格式化是否被接受；现在只有 `Blocks` 忽略空白，`Section`（摘要比较）与 `File`（tree 比较）不忽略。期望三种方式对「只改空白 / 换行 / 行尾逗号」给出同一个结论（接受），其余改动照旧拒绝。
- 协议正文有改动，按 `protocol/CHANGELOG.md` 的规矩发新版本（独立的 `## protocol/vN` 标题，`clarify` 类）。金样同步。
- 已经固定在旧协议版本上的在跑任务不迁移。

## 实际行为

见现象。会话读到的文字比内核的实际口径更严，三轮修复预算被一个机械的格式问题耗尽。

## 环境

main `d9ad281`；`protocol/prompts/{locate,fix,fix-review}.md`、`protocol/bug-protocol.md`、`crates/automed/src/scheduler.rs`（`regression_protection`、`blocks_kept_in_order`、`section_digest`、`guard_bug_regression`）。

## 线索（可选）

- 回归测试可以放在 `crates/automed/src/scheduler.rs` 的单测旁（三种保护方式各一条「纯格式化被接受、改断言被拒绝」），prompt 文字用金样 / `prompt_rendering.rs` 守住。
- 定位轮注意：本任务自己的回归测试提交前先跑 `cargo fmt --all -- --check`。
- 复现命令用仓库根目录可直接执行的形式，例如 `cargo test --offline -p automed --lib regression_format_only -- --test-threads=1`。
