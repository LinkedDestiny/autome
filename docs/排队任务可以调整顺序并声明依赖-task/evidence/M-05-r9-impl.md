# 实现 #9：rebase 集成复验（关联 M-05）

## 范围与状态

本轮简报及里程碑表均显示 M-01～M-06 已由审计关闭，没有开放里程碑；上一轮审计 M-06 通过，无退回缺陷。本轮按用户追加指令完成 rebase，不新建里程碑、不把已关闭项改回待审，也不自行关闭里程碑。保留 status: 已完成、current-milestone: 无、next-action: 无，implementation-round 更新为 9/30。

`git rebase main` 已完成，目标为本地 main `551497799dc51f77065a5796a9c9c0e9310c337a`；`git merge-base --is-ancestor main HEAD` 返回 0。未 fetch、未 merge、未推送、未切换工作分支。冲突解决已包含在 rebase 重放的提交中。

## 冲突与修复

- dispatch.rs：dashboard 同时保留主分支 config_warnings 与本任务 queued 投影。
- args.rs、render.rs：保留两侧新增测试；main.rs 同时保留 merge --cancel、合并状态后缀、配置警告与 after / queue 参数、排队顺序和依赖展示。
- technical-design.md：保留合并授权说明及依赖/队列接口说明。
- 回归发现旧测试 `after_create_reads_dependencies_and_input_boundaries` 比较 project.get 任务与 task.get.task 的完整对象，却未计入设计 §6 规定仅 project.get 任务新增的 queue_position。旧分支也有同一断言。调整预期：完整保留原任务字段，并要求草稿 queue_position 明确为 null；不删除字段断言、不改产品行为。这是本任务新增测试的契约修正，不修改原有需求/Bug e2e。
- 红：`cargo test -p automed --lib after_create_reads_dependencies_and_input_boundaries -- --nocapture`，1 个用例失败，唯一对象差异为 queue_position:null。原始输出 `.autome/output/r9-after-create-red.txt`；首次 after_create_ 子集 5 个中 1 失败、4 通过，未继续 e2e。
- 上轮审计未用检查发现实现缺陷，因此「逐字搬入审计检查」不适用；本次失败本来就在项目测试体系内。

## 验收命令与结果

- `cargo test -p automed after_blocked_`：7 通过（lib 6 + e2e 1）。日志 `.autome/output/r9-after-blocked.txt`。
- `cargo test -p automed after_create_`：修正后 6 通过（lib 5 + e2e 1）。日志 `.autome/output/r9-after-create-green.txt`；上述红用例包含在其中，现已绿。
- `cargo test -p autome-cli deps_cli_`：7 通过（bin 3 + CLI 集成 4）。日志 `.autome/output/r9-deps-cli.txt`。
- `cargo test -p autome-cli --test cli`：28 通过。日志 `.autome/output/r9-cli.txt`。
- `cargo test -p autome-cli --bin autome merge_`：4 通过。日志 `.autome/output/r9-cli-merge.txt`。
- `cargo test -p automed --lib merge_view_dashboard_reports_authorization_waiting_and_warnings`：1 通过。日志 `.autome/output/r9-dashboard-merge.txt`。
- 最终验收共 53 次用例执行，去重 49 个用例，全部通过、0 跳过；deps_cli_ 的 4 个集成用例在全 CLI 中再执行了一次。对照上次 M-03 基线 13 个与 M-05 基线 31 个去重用例，共 44 个，本轮增加 5 个既有主分支兼容检查（CLI merge_ 4 + dashboard 1），没有新增或删除项目测试用例。对应契约子集计数差均为 0；未把首次失败/诊断重跑计入最终通过总数。
- `git diff --check`、main 祖先检查及状态块字段/六项里程碑状态检查均通过。

## 自审四项

1. 情形表逐行：CLI 创建带依赖、清空依赖、队首移动、ls 文本/JSON、status 依赖/排队段由 deps_cli_ 与全部 CLI 集成覆盖；合并授权参数、状态后缀、事件文案由 merge_ 单测覆盖；dashboard 排队顺序由 after_blocked_ 覆盖，配置警告和运行组授权/等待字段由 merge_view_dashboard_reports_authorization_waiting_and_warnings 覆盖。技术文档的两组接口说明均保留。
2. 失败分支：CLI 同时给列表与 --none、缺少参数、移动位置不唯一；RPC 实现后修改依赖、成环、自引用、不存在、跨项目、错误输入类型均由上述项目用例执行。外部服务不可达、读盘失败、超时不适用：本轮未修改这些路径，也未新增外部服务。
3. 全部迁移：沿用 after_blocked_ 的取消阻塞→清空后启动、失败→恢复→再失败及重复 tick 通知去重，after_create_ 的依赖 Done 后放行、设计期 Requeued 受约束/实现历史 Requeued 放行。CLI 的合并授权与状态渲染既有分支保留；本轮没有新增状态迁移。
4. 域边界：after_create_ 检查空依赖、单个 Done 依赖、8 个重复/8 个不同依赖、非法输入；CLI 检查空/互斥参数。草稿 project.get.queue_position=null 明确参与完整对象比较。闰日、跨年等日期域不适用，本轮不处理日期输入。

未运行全量门禁或宿主快检；由宿主按用户指令执行。未打开真实账本，测试沿用项目沙箱。
