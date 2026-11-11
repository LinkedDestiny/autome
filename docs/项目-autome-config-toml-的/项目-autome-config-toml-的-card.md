# 项目 .autome/config.toml 的 [loop] parallel 不生效

## 现象
autome-v2 的 `.autome/config.toml` 写了 `[loop] parallel = 2`，但调度器同时跑了 3 个任务（T-16、T-17、T-18）。账本里项目行的 `parallel_limit` 是 3，取自添加项目时的全局默认（`~/.autome/config.toml` 的 `parallel = 3`），之后仓库配置文件里的覆盖从未写进去。

## 复现步骤
1. 全局配置 `[loop] parallel = 3`，添加一个项目
2. 在项目仓库的 `.autome/config.toml` 写 `[loop] parallel = 1`
3. 连续新建两个任务

## 期望行为
项目配置文件里的 `[loop] parallel` 覆盖全局默认，第二个任务排队。`config.get` 显示的生效值与调度器实际使用的一致。

## 实际行为
两个任务同时开始；调度器用的是账本里缓存的 `parallel_limit`（3）。

## 环境
- autome-v2 main @ fd519b2，macOS（Darwin 27），已安装的 `~/.local/bin/autome` 由同一提交构建
- 项目 autome-v2 自身由 tmux 里的 `autome run` 驱动

## 线索
`crates/automed/src/scheduler.rs` 的 `fill_slots` 读 `project.parallel_limit`；`dispatch.rs` 添加项目时写 `global.loop_defaults.parallel`，只有 `config.set_loop` 会更新它。仓库配置文件的解析在 `config_io::parse_project`。
