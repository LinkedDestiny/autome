## 现象

T-29 修复后（合并后生效），`scheduler.rs::fill_slots` 对每个项目调用 `resolve_config(ctx, &project)?` 取并行上限。`resolve_config` 会读全局与项目 `.autome/config.toml`、扫描 skills 并做 `config::validate`；任何一步出错（某个项目的 config.toml 写坏、引用的 skill 不存在、项目目录被移走导致解析失败）都会让 `fill_slots` 整体返回 `Err`，`tick` 只把它记进 `report.errors`。

结果：`list_projects` 中排在这个项目之后的**所有其它项目**，本次 tick 都不再从队列启动任务；每个 tick 都一样，直到那个项目的配置被修好。修复前 `fill_slots` 只读数据库里的 `parallel_limit`，不会因此失败。

## 期望

- 一个项目的配置解析或校验失败，只影响它自己：该项目本次不启动排队任务（它的会话本来也起不来），错误记进 `report.errors`（带项目名），其它项目照常补位。
- 不改变 T-29 的语义：并行上限仍以当前配置文件为准。

## 复现思路

两个项目都有排队任务、并行上限都有空位；把排在前面的项目的 `.autome/config.toml` 写成非法 TOML（或 `[loop] parallel = "x"`）；跑一次 `tick`：后一个项目的排队任务应当被启动，现在不会。
