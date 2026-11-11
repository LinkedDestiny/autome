# M-05 r7 实现：rebase 冲突兼容修复

## 范围与状态

本轮简报没有开放里程碑；M-01 至 M-05 已经独立审计通过，上一轮审计没有退回缺陷。本轮按用户末尾的冲突处理指令，仅修复 `render.rs` 与本地默认分支 `main`（5a8cd57）的兼容差异。进入时工作区干净、`git ls-files -u` 为空、无冲突标记、没有正在进行的 rebase；因此不执行 rebase/切分支，而在当前分支合入冲突涉及的默认分支代码。

对照发现本分支缺少 main 中单仓库 `merge.rebased` 的专用显示分支及两条原有测试。逐字恢复这 42 行，保留本任务的熔断失败原因、日志路径及 rerun/extend 提示。修复后 `git diff main -- crates/autome-cli/src/render.rs` 仅剩本任务新增行，没有删除默认分支代码。本轮未新建或重新关闭里程碑；沿用审计 #5 的关闭结论，状态块轮次更新为 7/25，next-action 保持无。

## 回归红绿证据

- 原样搬入 main 两条 `rebased_event_*` 测试，先运行 `cargo test -p autome-cli rebased_event`：2 项，1 通过、1 失败。单仓库实际输出含空仓库名 ` → main`，与 `T-14 已自动 rebase 到 main @ 5c8631f6a5` 不符。原始输出：`.autome/output/r7-red.txt`。
- 恢复 main 的单仓库显示分支，再运行同一命令：2 项全部通过。原始输出：`.autome/output/r7-green.txt`。
- `cargo test -p autome-cli gate_same_failure`：2 项全部通过，较上一轮该子集 2 项差 0。原始输出：`.autome/output/r7-gate.txt`。

## 自审清单（逐项执行）

1. **情形表逐行**：本轮只涉及 §5 两个 CLI 读者。`failure()` 对 count 1/3/10 均输出原因、摘要和中文日志路径；`next_step()` 对 gate/merge_gate/fix_gate 均输出原节点 rerun 与 extend，两个已有测试通过。冲突另一侧：空仓库名的单仓库、单个有名仓库、两个有名仓库逐项通过恢复的原始测试。
2. **失败分支**：熔断失败显示由 `gate_same_failure_explains_the_reason_and_log` 实跑；空仓库标签错误由上述红绿回归实跑。授权拒绝、读盘失败、超时、外部服务不可达不适用于本轮纯字符串渲染修复，无 I/O 或这些失败分支。
3. **全部迁移**：不适用，本轮不修改状态机。涉及的三个失败节点 gate/merge_gate/fix_gate 的继续操作提示均在原有测试循环中实跑通过。
4. **输入域边界**：count 下界 1、默认 3、上界 10 与中文摘要/路径由已有用例覆盖；单元素空仓库名、单元素有名仓库、多仓库、长 SHA 截断由恢复的两条测试覆盖。日期、闰日、跨年不适用，本轮没有日期输入。

## 其他验收

- `npm test --prefix apps/desktop`：总数 105，104 通过、0 失败、1 跳过；与上一轮总数差 0。输出 `.autome/output/r7-desktop.txt`。
- `cargo clippy --all-targets -- -D warnings`：通过（`.autome/output/r7-clippy.txt`）。
- `cargo fmt --all -- --check`：通过，无输出（`.autome/output/r7-fmt.txt`）。
- `git diff --check`：通过。
- `rg -c same_failure_limit README.md` = 2；`rg -c 熔断 docs/development/technical-design.md` = 1。
- `git diff --numstat $(git merge-base HEAD main) -- crates/automed/tests/end_to_end.rs apps/desktop/test`：159/0、17/0，原测试无删除。本轮产品文件仅 render.rs 新增 42 行，其中默认分支原有测试原样恢复，不修改既有测试。
- H-01 仍待用户合并前人工确认；本轮没有进行真实桌面操作。宿主门禁由 Autome 调度，本轮未自行运行或更改 `ci-local.sh`。
- `cargo test --workspace`：退出码 0；总数 1133，1132 通过、0 失败、1 忽略（真实 T-14 日志用例）；较上一轮总数 1131 增加 2，全部来自原样恢复 main 的两条 `rebased_event_*` 回归测试，没有删除或新增跳过。原始输出 `.autome/output/r7-workspace.txt`。
