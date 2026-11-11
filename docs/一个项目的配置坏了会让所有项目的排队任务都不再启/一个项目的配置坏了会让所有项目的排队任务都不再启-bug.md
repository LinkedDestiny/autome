# 单项目配置解析失败阻断其它项目补位

status: 已定位
fix-round: 2/3
复现命令: cd crates/automed && cargo test --offline --lib scheduler::tests::tick_isolates_invalid_project_config_and_still_fills_other_projects -- --exact --nocapture
根因: fill_slots 在逐项目循环中用 resolve_config(ctx, &project)? 读取上限，单项目配置错误提前返回整个函数，tick 仅记录错误而不继续处理后续项目。
复核: 待复核
repos: crates/automed

## 现象

一个排在前面的项目存在非法 TOML 时，后续健康项目即使有排队任务和空闲名额，也不会在本次 tick 启动。配置错误持续存在时，后续 tick 会再次在相同位置返回。影响范围是排序中出错项目之后的项目，并非所有项目在任何排序下都不能启动。

预期是仅跳过配置失败项目，将带项目名的错误记录进 `report.errors`，其它项目继续按各自当前配置的并行上限补位。

## 复现

在当前 worktree 根目录执行状态块中的命令。依赖本机 Rust/Cargo、Git 和已缓存的依赖；`--offline` 禁止依赖下载。测试使用临时目录、真实 Git 仓库和内存数据库，沿真实 `tick` 调度路径执行；启动器使用现有 Dry 模式，不启动外部 Agent，不调用模型服务。

新增测试位于 `crates/automed/src/scheduler.rs` 的 `#[cfg(test)] mod tests`，没有修改产品逻辑。

1. 建立两个项目，以固定且不同的 `added_at` 保证坏项目排在健康项目之前，并断言实际顺序。
2. 两个项目均有排队任务、没有占用名额。配置文件并行上限为 1，数据库上限为 3；健康项目放入两个排队任务。
3. 先执行两个项目配置正常的对照组：两边都启动会话，健康项目第二个任务仍排队，证明夹具可启动且上限来自配置文件。
4. 在全新夹具中，仅把首个项目配置写为 `[loop` 后换行 `parallel = 1`，即缺少表头右括号，再执行一次 tick。
5. 断言健康项目首个任务进入 Intake、创建会话且出现在 `tasks_started`；第二个任务仍排队。另断言坏项目保持排队、没有会话，以及错误包含独立于目录名的项目显示名。

实跑结果：正常配置对照通过；非法 TOML 组在健康任务状态断言失败，实际 `Queued`，预期 `Active { node: Intake }`。`TickReport` 中 `tasks_started: []`、`skipped: false`，`errors` 包含 `invalid table header`。Cargo 退出码为 101，属于目标行为断言失败，不是超时或环境错误。

原始日志：`.autome/output/config-isolation-invalid-toml.log`；复跑日志：`.autome/output/config-isolation-repeat.log`。日志不进入提交。

## 根因分析

- `crates/automed/src/store.rs::list_projects` 按 `added_at DESC` 返回项目；测试使用固定时间确保顺序，无需等待或依赖时间碰巧不同。
- `crates/automed/src/scheduler.rs::fill_slots` 在项目循环开头调用 `resolve_config(ctx, &project)?`，配置加载、解析或校验错误会提前返回整个 `Result<Vec<String>>`。任务启动内部已有的错误隔离无法覆盖这个更早的项目级错误。
- `resolve_config` 加载全局与项目配置、扫描 skills 并校验。非法 TOML 经 `config_io::load_project` 和 `parse_project` 返回错误。
- `tick_locked` 对 `fill_slots` 的 `Err` 只追加到 `report.errors`，不会恢复后续项目的遍历。因此每次仍先遇到坏项目时，都无法为后续健康项目补位。
- 此外，若错误发生在某些项目已经补位之后，局部 `started` 向量也会随错误返回而丢失，报告无法反映那些已启动任务；这是控制流分析结果，本轮测试主要锁定任务卡要求的后续项目阻塞。

任务卡复现示例有两处需要限定：`parallel = "x"` 是合法 TOML，当前 `parse_project` 使用 `as_integer`，会把该字段视为未提供，而非报错；实跑该输入没有触发目标阻塞，故最终测试使用语法非法 TOML。配置文件不存在时 `load_project` 返回默认覆盖，因此“目录移走”也不必然在这一步报错。本轮未声称验证所有配置失败变体。

## 修复说明

第 2/3 轮复查：保留第 1 轮项目级配置错误隔离实现，未修改定位轮回归测试，也未修改门禁配置。原复现命令再次通过（1 passed、0 failed）；完整 `cargo test --workspace --all-targets` 本轮通过，未复现宿主汇总中的 Test 失败。宿主提供的末尾日志没有具体失败用例，不能据此推断新的产品根因。

Format 再次失败，全部差异位于定位轮回归测试，均为 rustfmt 换行要求。当前指令明确禁止修改该测试，因而未应用格式化，也未使用跳过或放宽格式检查的方式绕过门禁。已请求仅格式化的例外许可；截至记录时未收到答复，仍按原限制执行。

本轮按宿主相同组合命令执行原复现命令及 `sh scripts/ci-local.sh`，最终退出码 1，门禁用时 652 秒。Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 通过；Format 和 Web test 失败。Web test 为 4 个文件失败、11 个用例失败、271 个通过、19 个跳过，日志包含 `listen EPERM: operation not permitted 127.0.0.1` 与 `EMFILE: too many open files, watch`，当前沙箱不允许本地监听。未修改或弱化相关测试，亦未调整门禁；本轮不能声称门禁全部通过。

`git diff --check` 通过；产品代码及定位轮回归测试保持不变。本轮完整门禁日志：`.autome/output/fix-round-2-gate.log`（不进入提交）。

以下保留第 1/3 轮记录：

第 1/3 轮将补位逻辑中的配置解析错误隔离在项目级：解析失败时向 `report.errors` 写入项目显示名及错误，跳过该项目并继续遍历。配置失败不会再丢失此前已启动任务的列表；健康项目仍使用当前解析配置中的并行上限。保留测试使用的 `fill_slots` 入口，委托同一实现；定位轮回归测试逐字比对无变化。

验证结果：

- 修复前执行原复现命令，退出码 101，健康任务实际为 `Queued` 而非 `Active { node: Intake }`。
- 修复后执行同一命令，退出码 0，1 个测试通过、0 失败；同时覆盖正常配置对照、坏项目保持排队、健康项目补位、配置上限和项目名错误报告。
- `git diff --check` 通过。
- 配置门禁 `sh scripts/ci-local.sh` 已执行完成，退出码 1，用时 950 秒。Clippy、工作区 Test（含 automed 615 个单元测试）、独立 End to end（81 个用例）、Adapter flags、Desktop、Contract、Web lint/typecheck/build 以及依赖安装步骤通过。
- Format 未通过：rustfmt 要求调整定位轮回归测试内的换行，差异全部位于该测试。遵守本轮不得修改定位轮回归测试的约束，保持原文，未运行格式化写入或跳过门禁。
- Web test 未通过：4 个测试文件失败，11 个用例失败、271 个通过、19 个跳过；日志为 `listen EPERM: operation not permitted 127.0.0.1`，并伴随 `EMFILE: too many open files, watch`。当前受限环境无法完成本地 HTTP/WebSocket 验收，未改动或弱化相关测试；需由具备本地监听权限的宿主复跑。因此本轮不能声称项目门禁全部通过。

日志保存在 `.autome/output/fix-round-1-before.log`、`.autome/output/fix-round-1-after.log` 和 `.autome/output/fix-round-1-gate.log`，不进入提交。

## 复核意见

待独立复核。第 2/3 轮原复现命令及完整 Rust Test 通过；项目门禁仍受 Format 与 Web test 阻塞，详见修复说明。
