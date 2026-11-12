# 复现命令写成 `cd 子目录 && … 相对路径` 时，定位轮新增的复现脚本不被认作回归测试，修复轮做完才报「回归测试保护集为空」

## 现象

T-56（2026-11-12）：定位轮新增复现脚本 `apps/web/scripts/notifier-live-regression.mjs`，Bug 文档的复现命令是

```
cd apps/web && node --test scripts/notifier-live-regression.mjs
```

复现检查正常通过（命令确实失败），修复轮也正常做完并提交。修复轮结束时任务却「失败（修复）：协议失败：回归测试保护集为空：定位轮必须新增或修改测试文件」。修复本身是对的（复现命令在修复后的提交上通过），而且修复轮没有碰那份脚本。

## 复现步骤

1. Bug 任务的定位轮新增一个不在 `test/`、`tests/`、`__tests__/` 下、文件名也不带 `test` / `spec` 标记的脚本（如 `apps/web/scripts/x-regression.mjs`）；
2. 复现命令写成 `cd apps/web && node --test scripts/x-regression.mjs`（路径相对 `cd` 之后的目录）；
3. 复现检查通过后进入修复轮，修复轮只改产品 / 既有测试并提交；
4. 修复轮结束 → 协议失败「回归测试保护集为空」。

## 期望行为

- `regression_protection`（`crates/automed/src/scheduler.rs`）判断「复现脚本」时，要认得出复现命令里**相对 `cd <目录>` 的路径**：命令里出现 `cd <dir> &&`（含括号包起来的 `( cd <dir> && … )`）时，把之后的路径按 `<dir>/` 解析再与仓库相对路径比较。上例应得到 `protect = "file"`。
- 保护集为空应当在**定位轮结束、写 `bug.regression_snapshot` 的那一刻**判出，而不是等修复轮花完一轮才报；原因文案指向定位轮，并写明内核认测试文件的规则（目录名、文件名标记、或复现命令里出现的脚本路径）。此时任务按现有「定位轮产物不合格」的路径处理（设计定：退回定位并带注入，或失败等人），不消耗修复轮次。
- 已经在修复轮才发现的旧快照（升级前留下的空快照）保持现在的报错，不改。

## 实际行为

脚本路径只与完整命令做子串匹配，`cd` 之后的相对路径匹配不上；空保护集在修复轮结束才报，报错落在修复轮头上。

## 环境

main `782e261` 之后；`crates/automed/src/scheduler.rs` 的 `regression_protection`（`script` 分支的 `command.match_indices(path)`）与修复轮守卫里的 `files.is_empty()` 检查。

## 线索（可选）

- 现有复现命令里大量使用 `cd crates/automed && cargo test …` 这种写法；cargo 用例因为文件在 `tests/` 下走的是 `named` 分支，所以一直没暴露。
- e2e 夹具可照 `bug_regression_guard_protects_only_the_added_tests_of_an_existing_test_file` 的写法。
