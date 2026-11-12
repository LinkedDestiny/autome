# M-05 · 实现 #6

## 范围与修改

仅推进 M-05。CLI `failure()` 与桌面 `failureReason()` 显示连续次数、指纹摘要和日志路径；CLI `next_step()` 根据失败的 gate / merge_gate / fix_gate 给出 rerun 与 extend 命令。README 配置示例和 technical-design §8.1 说明默认值、范围、按节点熔断及恢复行为。没有改 JSON-RPC、桌面按钮、门禁执行逻辑或 ci-local.sh。

上一轮审计 M-04 通过，没有退回缺陷，也没有必须搬入的审计缺陷检查。本轮仍先添加项目测试再实现，以旧实现作负向对照。

## 红绿证据

- `cargo test -p autome-cli gate_same_failure`：旧实现 2 个失败（返回原始 kind、恢复提示只有 status）；补上分支后 2 个通过。日志 `.autome/output/m05-cli-red.txt`、`m05-cli-green.txt`。
- `node --test apps/desktop/test/labels-failure.test.js`：旧实现新增用例失败，实际值为原始 gate_same_failure；补分支后包含于桌面全量通过。日志 `.autome/output/m05-label-red.txt`、`m05-desktop.txt`。

## 标待审之前的自审清单

1. **情形表逐行**：设计 §5 三行全部检查：CLI 失败文案严格相等，包含中文摘要与中文日志路径；CLI 恢复提示逐个检查 gate / merge_gate / fix_gate；桌面文案严格相等，包含次数、摘要、日志路径；README 配置与说明、技术设计熔断说明均已更新，grep 命中分别 2 与 1。不新增 RPC 或改参数，桌面原有按钮保持原样。
2. **失败分支**：M-05 只有 gate_same_failure 显示分支，两种读者均已红绿验证。拒绝授权、读盘失败、超时、空输入、外部服务不可达不适用于本轮读者（只格式化已有 JSON，无 I/O 或外部服务）；它们不产生本轮新增分支。底层已实现的配置拒绝、超时、日志读取边界与恢复拒绝仍由 workspace 原有测试覆盖。
3. **全部迁移**：M-05 不修改状态机，无新增迁移；恢复提示的三个门禁节点已逐个执行断言。底层熔断与 rerun / extend、错误节点拒绝、零追加拒绝、原有通过路径由 workspace 全量复验；未用显示单测冒充状态机或端到端验收。
4. **域边界**：两种读者分别执行 count=1、3、10，覆盖配置允许的两端和默认值；CLI 包含中文摘要、中文路径。日期、闰日、跨年、前导零不适用：本轮不解析日期或数字字符串；空摘要/日志不是本里程碑新增输入域，不追加新产品规则。

## 旧测试不改核对

对 `git merge-base HEAD main` 的累计差异逐条检查：
- `end_to_end.rs` 为 159 增、0 删；新桌面测试仅新增，已有桌面测试零改动。
- task.rs 唯一删除行是原第 930 行 ExtendBudget 生产分支匹配。
- scheduler.rs 删除的是原第 2847、2848、2850–2858 行 run_gate 内 append_event 的生产代码，改为带新字段的事件写入；不在测试模块。
- bug_loop.rs、config.rs、config_io.rs、gate.rs、render.rs 没有删除行。
- 初次辅助扫描把 scheduler.rs 第 975 行字符串常量中的 `#[cfg(test)]` 当成测试起点；人工核对删除行位于 run_gate，实际测试模块在其后，排除误报。逐行原始差异见 `.autome/output/m05-test-diff.txt`，删除行清单见 `m05-deletions.txt`。

## 限制

桌面基线与本轮均有 1 个 Electron DOM harness 跳过，原因是 electron binary not found；没有安装依赖或改测试规避。H-01 真实桌面失败卡片与点击恢复未取证，保留原人工验收清单。宿主 `sh scripts/ci-local.sh` 按任务约定交由 Autome 后续运行，本轮未自行启动下一会话。

## 最终验收与用例数量

- `cargo test -p autome-cli gate_same_failure`：2/2 通过，比上一轮该筛选 0 个增加 2 个，即失败文案与恢复命令两个新增测试。
- `npm test --prefix apps/desktop`：总数 105，104 通过、0 失败、1 跳过；本轮开始实跑基线总数 104（103 通过、1 跳过），差 +1，仅来自 labels-failure.test.js。原始日志 `m05-desktop-baseline.txt`、`m05-desktop.txt`。
- `cargo test --workspace`：总数 1131（包含忽略测试），1130 通过、0 失败、1 忽略（t14_gate_logs）。上一轮 M-04-r5-audit 基线为 1128 通过 + 1 忽略，即总数 1129，差 +2，恰为本轮 CLI 新增测试；domain 374、automed 库 618、end_to_end 83、CLI 21+11、prompt_rendering 16、real_cli 7。与设计预期一致，无整层缺失。退出码 0，原始日志 `.autome/output/m05-workspace.txt`。
- `cargo clippy --all-targets -- -D warnings`：退出码 0，无警告（只有正常编译进度）；`cargo fmt --all -- --check`：退出码 0、空输出。日志 `m05-clippy.txt`、`m05-fmt-check.txt`。
- `grep -c same_failure_limit README.md` 为 2；`grep -c 熔断 docs/development/technical-design.md` 为 1。
- `git diff --numstat $(git merge-base HEAD main) -- crates/automed/tests/end_to_end.rs apps/desktop/test`：端到端 159/0、新桌面测试 17/0（增/删），所有第二列均为 0；`git diff --check` 通过。

结论：本轮可自动执行的 M-05 验收通过，四项自审完成，提交待审；未将里程碑标为已完成。
