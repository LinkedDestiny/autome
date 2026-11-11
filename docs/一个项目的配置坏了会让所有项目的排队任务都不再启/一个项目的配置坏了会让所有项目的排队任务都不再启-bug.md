# 单项目配置解析失败阻断其它项目补位

status: 已定位
fix-round: 0/3
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

定位轮未修复产品代码。后续修复应将配置失败隔离在项目级，保留错误和已启动结果，并继续遍历其它项目；并行上限仍应来自当前解析后的配置。

## 复核意见

待宿主复现检查及后续独立复核。本轮回归测试按要求保持失败。
