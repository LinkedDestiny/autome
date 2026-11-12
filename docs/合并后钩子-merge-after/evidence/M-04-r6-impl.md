# 实现 #6：已关闭任务的 rebase 与集成复验

## 范围与决定

开场简报没有开放里程碑；审计 #4 已关闭 M-04，四项均已完成，未退回缺陷。本轮依用户追加要求执行 `git rebase main`，不新增里程碑、不自行关闭里程碑。起初保留原关闭状态；完成 rebase 后实际复验出现阻塞，故最终仅将 M-04 恢复为 `开放`，状态设为 `实现中`，下一步明确环境复验，轮次为 `6/20`。没有达到完整验收前不标待审；非审计认定的产品缺陷，不增加 reopen。此文件归在 M-04 全量回归项下，供后续独立核验。

## 冲突处理

- 默认分支为 main（origin/HEAD 指向 origin/main），目标提交 `90802f65af43734a1235d9f51ad7f10d7d832a8d`；rebase 后 HEAD `2bc1df3f2f473ac920b81b1d962eaa34b8bd0370`。
- config.rs：同时保留 main 的 gate_run、gate_concurrency、gate_reuse_for_main_check 与分支 merge_after 类型、解析后配置、resolve 及双方测试；保持 GateRun 的 serde 属性归属。
- config_io.rs：保留完整 main 配置解析，增加原分支 merge 解析及四个原测试，不覆盖门禁新增测试。
- 后续提交还在 render.rs、end_to_end.rs、apps/desktop/test/dom.test.js、README.md 冲突：保留两侧测试和事件打印，Electron 保留 main 的 signal/error 诊断及分支 assert.ifError，README 同时保留新门禁选项与 merge 示例，移除过时的“三件事”计数。
- 四个 Rust 冲突文件逐一比较两侧函数名集合，当前文件包含两侧全部函数；未删测试。rebase 正常完成，没有 merge 提交、未切分支、未推送。
- 原始记录：`.autome/output/r6-rebase*.txt`、`r6-conflicts*.txt`、`r6-config-final.diff`。

## 验收命令与结果

- `cargo test --workspace --all-targets -- --list`：1385 项，merge_after 23 项。相对上一轮 1219 增加 166，全部来自 main：CLI bin +9、CLI integration +3、domain +23、automed lib +88、e2e +31、prompt_rendering +12；其他目标不变。本轮无新增或删除用例；两个用例仅更新夹具，原断言保留。
- 完整 CI 命令：`sh scripts/ci-local.sh > .autome/output/r6-ci.txt 2>&1`（结果见下方最终记录）。其中包括 fmt、clippy、全量 Rust、Desktop、Contract、Web lint/typecheck/test/build。
- `git diff --exit-code main -- .autome/`：退出 0，本仓库配置未改。
- rebase 中途曾提前启动 cargo test，因尚未解决的 Rust 冲突标记编译失败（`.autome/output/r6-all.txt`），不作为最终验收或缺陷红测；最终验收只采用 rebase 完成后的 CI。
- 上轮无审计缺陷退回，因此“逐字搬入审计检查、红绿复现”不适用。

## 自审清单

1. **情形表逐行**：M-04 文档示例和说明随 main 合并仍保留；M-01 的工作区/无提交/门禁失败/门禁跳过/未配门禁/门禁通过六种决策由原有 domain 三个 plan 测试覆盖。配置与主检出执行情形由原有 23 个 merge_after 测试复验。
2. **失败分支**：非法表、非字符串、非法超时由配置测试覆盖；钩子非零、超时 124、main gate 失败、setup 失败、事务插入失败由现有测试复验。拒绝授权、外部服务不可达不适用于此次纯配置冲突合并；没有新 I/O 失败分支。读盘失败未新增专门注入，本轮未改动文件 I/O 实现。
3. **全部迁移**：此次 rebase 不改变钩子状态机。现有测试检查 Running→Pending、Done→收集→清理、结论后重入防重复、启动后补事件、互斥等待→启动、Stop→恢复重跑及跳过→清理。Pause、Cancel、Vanished 与配置移除分支未单独新增运行检查；相关实现无冲突且未改动，不把本轮复验描述为新增的完整生命周期证明。
4. **域边界**：现有配置测试覆盖无表、无 after、空串/空白、去首尾空白、缺省 10、1、u32::MAX、超限截断、i64::MAX、0、负数、字符串/浮点/布尔/数组/表；日期与跨年不适用，钩子配置没有日期输入域。


## main 新语义的集成修复（红 → 绿记录）

首次完整 CI 的 Rust e2e 为 142 项，140 通过、2 失败：E1 在寻找 MainCheck 的 `gate.result` 时 unwrap(None)，E4 期望一条 main_gate_failed 警报但实际为 0。根因是 main 新增默认 `reuse_for_main_check = true`，在树一致时复用前面的成功门禁，不重新执行 main-should-fail 脚本。属于测试前置条件过时，没有证明钩子实现出错。

这两个原有测试就是判别检查，未删除、未放宽断言；将其 `run_gated_task` 改为 `run_gated_task_with(..., false, Some(false))`，明确要求重新执行 MainCheck。其余钩子 e2e 保留默认复用，继续覆盖新默认行为。设计验收契约同步明确该前置条件。

红：`.autome/output/r6-ci.txt`；首次过滤回归被 SIGKILL，未产生用例结果（`r6-hook-green.txt`），不算通过。随后串行完整 Rust 验收记录为 `.autome/output/r6-final-all.txt`。最终 fmt 单独复验退出 0（`r6-final-fmt.txt`）。

## 完整 CI 阻塞

- 首次 CI 退出 1：Format、Clippy、Contract、Web lint/typecheck/build 通过；Rust 为上述两个已修夹具失败；Desktop 105 项（104 通过，1 失败），Electron `SIGABRT`、未启动真实 DOM harness。
- Web 总数 385：348 通过、18 失败、19 跳过；45 文件中 7 失败、38 通过。明确错误为 `listen EPERM: operation not permitted 127.0.0.1` 和 `EMFILE: too many open files, watch`。不削弱断言、不将环境失败改为 skip。
- 第二次全量 Rust 在 automed lib 停止：745 项，744 通过、1 失败，`gate_queue_upgrade_collects_two_old_jobs_without_killing_them` 的 `j.exists()` 断言失败；该用例来自 main，首次全量通过，代码本轮未改。不能用首次通过掩盖第二次失败；单测重试另记。
- 最终全量没有通过，因此不声称 1385 项全部执行成功。1385 是清单总数，第二次在 lib 失败后尚未执行 e2e/prompt_rendering/real_cli。
- 本次产出是完成 rebase、恢复双方代码与测试、修复两个过时夹具及明确剩余验证阻塞。M-01–03 保留先前审计关闭；M-04 开放等待环境复验，不改为协议失败。

## 最终定向复验

- 删除并重建已连续被 SIGKILL 的单个 e2e 编译产物后，`cargo test -p automed --test end_to_end merge_after` 退出 0：8 通过、0 失败、134 过滤；总数仍 142。日志 `.autome/output/r6-hook-rebuilt.txt`。E1、E4 原断言均绿，其他六个钩子 e2e 同时通过；未将 SIGKILL 原因推断为已证实。
- `cargo test -p automed --lib gate_queue_upgrade_collects_two_old_jobs_without_killing_them` 退出 0：1 通过、744 过滤（`.autome/output/r6-main-retry.txt`）。该结果仅说明单独重试通过，完整运行中的失败仍需复核。
- `git merge-base --is-ancestor main HEAD`、`git diff --check` 均退出 0；README `[merge]` 示例、After merge 条目及环境变量说明均保留。
- 原有 23 个钩子用例：domain 4、automed lib 10 在完整运行中通过，e2e 8 在最终重建后通过，CLI 1 在完整运行中通过。用例总数与上一轮的增量仍为 +166，来源如上。
- 证据、状态与 retro 更新后提交当前分支；不启动后续会话。
