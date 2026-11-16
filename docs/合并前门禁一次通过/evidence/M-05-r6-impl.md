# M-05 实现轮 #6

结论：待审。75 个主验收用例全部通过（较上一轮同范围 +10），四项自审完成，设计点名的负向对照均能变红并已还原。

基线 HEAD：`734d9df5`。本轮仅推进 M-05；上一轮 M-04 审计通过，没有退回缺陷检查需要逐字迁入。负向变异均为本轮主动构造，不冒充上一轮审计回归。

## 实现与范围

- `[gate] quick_session` 为可选字符串模板：空白忽略；非空要求有效 quick，且恰好一个 `{round_start}`。存入 QuickGate 的可选 session 字段，旧序列化数据缺省为 None。
- 需求实现轮与 Bug 修复轮在写、提交简报之前捕获 start_heads；简报、提示词、session.started 和 gate_round_env 使用同一值。已有两条起点端到端测试升级为四处一致性检查，未重复新增同功能用例。
- 单仓库且有起点时使用模板；未配置模板时生成带正确单引号转义的 `AUTOME_ROUND_START=<sha> sh -c '<quick>'`。工作区或无起点维持原句。Bug 修复轮此前无简报，仅在本轮快检说明存在时新增 `brief/impl-<轮次>.md`，提示词明确指向它。
- 全部宿主门禁节点导出 AUTOME_GATE_DEADLINE = 启动时 Unix 秒 + 生效超时。需求 Gate 使用 quick_timeout_minutes；Bug FixGate 使用完整门禁 timeout_minutes，MergeGate 同样使用完整门禁超时。
- 快检相关阶段结束即冻结汇总，浏览器准备/执行不计入；超预算时列出最后执行步骤的开始偏移、耗时和后续预算拒绝步数。零预算没有准入步骤时不虚构最后步骤。Web related 保持逐文件准入。
- start_heads 使用方已检索复核（`.autome/output/m05-start-users.txt`）：gate_round_env 按 session.started 读取；integration 的 repo_range 继续排除任务文档目录，简报提交不会扩大集成代码范围。
- 三份快检 golden 已复核：impl、fix 两份随起点说明更新；fix-review 不属于实现/修复轮，保持原文。README 记录配置、截止时间和准入预算语义。

## 命令与用例数

结果：主验收范围 75 / 75 通过，0 失败、0 跳过；直接相关补充范围也全部通过。最终小调整仅将 deadline 计算移到 gate_round_env 读取之后，避免 Git 查询消耗预算，该调整重跑 gate_deadline；完整库 1044 的第二次通过对应这一独立语句移动之前。


本里程碑验收范围共 **75 个**，相同功能范围的上一轮基线 **65 个**，差 **+10**：配置 +3、launcher +3、prompt +1、deadline +1、ci-quick +2。起点端到端的 2 个来自既有用例的强化与重命名，数量差为 0（旧名称 exports_round_start 改为 gate_quick_round_start）。设计中的 ci-quick 基线 15 已复核；prompt 上轮 48。

| 验收命令 | 本轮 / 基线 | 原始输出 |
|---|---:|---|
| `cargo test -p automed --lib config_io::tests::gate_quick_session` | 3 / 0 | `.autome/output/m05-config.txt` |
| `cargo test -p automed --lib launcher::tests::round_gate_line` | 3 / 0 | `.autome/output/m05-launcher.txt` |
| `cargo test -p automed --test prompt_rendering` | 49 / 48 | `.autome/output/m05-prompts.txt` |
| `cargo test -p automed --test end_to_end gate_quick_round_start` | 2 / 2 | `.autome/output/m05-start-final.txt` |
| `cargo test -p automed --test end_to_end gate_deadline` | 1 / 0 | `.autome/output/m05-deadline-final.txt` |
| `node --test scripts/test/ci-quick.test.mjs` | 17 / 15 | `.autome/output/m05-quick.txt` |

直接相关补充范围：`cargo test -p automed --lib` 1044 / 1038（+6，为上表 config/launcher）；`cargo test -p autome-domain --lib` 487 / 487；`node --test scripts/test/*.test.mjs` 70 / 68（+2，为上表 ci-quick）。全部命令去重后 1653 / 1643，差 +10；嵌套执行、筛选重跑、golden 更新与负向变异不重复计数。输出分别为 `m05-lib-final.txt`、`m05-domain.txt`、`m05-scripts.txt`，均在 `.autome/output/`。

没有运行项目快检或全量门禁；脚本测试只在原有临时夹具中执行替身门禁。宿主的快检、合并前全量由 Autome 后续运行。

## 标待审之前的四项自审

1. **情形表逐行**：C-Q2 配模板 / 不配模板 / 工作区与无起点三行，分别由模板测试、真实 sh 执行转义命令测试、无起点保持原句测试检查。需求实现与 Bug 修复均检查简报、提示词、事件、宿主环境四处一致。C-2 正常 / 超预算 / 零预算分支由 ci-quick 原有边界与新增汇总检查覆盖；已有逐文件预算测试补断言实际最后步骤、耗时及拒绝数，新用例实际运行 quick-related 对两个文件生成两个候选且每候选只有一个文件。
2. **失败分支**：缺 quick、quick 为空、模板无占位符 / 两个占位符、模板非字符串均拒绝；模板空串与空白忽略。命令中的单引号与环境引用经过真实 sh 执行验证。脚本既有非法预算/起点、报告路径不可写、相关测试失败分支照跑。授权拒绝、外部服务不可达不适用：本里程碑没有新增授权或外部服务调用；读盘失败未新增独立行为，沿用原有简报与 git 错误处理；超时按两种门禁生效预算验证截止值，误差不超过 2 秒。
3. **全部迁移**：需求 Implement → Gate → Audit 与 Bug Fix → FixGate → FixReview 两通道都用实际宿主替身运行，后续 MergeGate 一并核对 deadline；需求 Gate 2 分钟、FixGate/MergeGate 7 分钟。快检未启动 → 准入 → 运行结束 → 预算拒绝余项 → 冻结汇总的路径，以及正常结束、零预算不准入，都由脚本测试执行；浏览器在冻结之后，不改变阶段时间。没有新增任务状态或调度迁移。
4. **输入域边界**：模板 0 / 1 / 2 个占位符、空串 / 空白、非字符串；shell 单引号；SHA 使用 git 提供的完整值；预算 0、前导零、负数、非法字符、非整数、安全整数最大值和溢出沿用既有 ci-quick 边界。Web 候选单文件与多文件均检查。日期、闰日、跨年不适用：无日期输入，deadline 使用 Unix 秒加生效超时。

## 负向对照与恢复

- 放行缺少占位符的模板：config 3 中 1 红，`.autome/output/m05-negative-config.txt`。
- session.started 改为读取简报提交后的 HEAD：两条四处一致性检查均红，`.autome/output/m05-negative-start.txt`。该变异模拟原先起点晚读导致载体不一致。
- 所有门禁错误地用 quick 超时：deadline 检查红，`m05-negative-deadline.txt`；另精确变异仅让 Bug FixGate 用 quick 超时，仍红，`m05-negative-fix-deadline.txt`。其余节点保持正常，能单独区分设计点名的 Bug 错误。
- 变异已恢复，恢复后重新执行主验收与完整 automed 库，随后仅移动 deadline 计算语句并再次通过其端到端检查；负向运行不计入成功验收数。

## 其它检查

`cargo clippy -p automed --lib -- -D warnings` 通过（`.autome/output/m05-clippy.txt`）；`cargo fmt --all`、`sh -n scripts/ci/lib.sh` 与 `git diff --check` 通过。脚本 70 用例总耗时约 474 秒、单独快检测试 17 用例约 335 秒，均无取消或跳过；未把运行耗时当作失败或降级依据。
