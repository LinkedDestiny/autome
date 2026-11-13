# M-14 · 实现 #15

范围：只推进 M-14（G-R1/R3/R4/R5/R6，G-V1/V3/V4）；implementation-round 为 15/80。上一轮 M-13 审计通过，没有退回的缺陷检查需要逐字搬入。

## 实现与修复

- project.add / project.probe 共用 add_rejection，返回四种 kind、原文 message、关联 project_id；保留 add 创建目录的时机、原拒绝顺序与移除后复用 ID 的行为。probe 无写盘，规范化现有路径、符号链接和不存在子路径的父目录。
- config.get 独立补齐 ResolvedConfig 全部 14 个顶层键，不修改共享 serde；defaults 按项目覆盖与全局文件实际键判断，显式等于默认值也不误标。聚合字段整体缺省用顶层名，loop_defaults 部分覆盖用字段路径；角色存在覆盖时不标成整体缺省。
- dashboard.get 新增 projects 列表及每项目 config_error；project.get 坏配置返回 config=null 与原始错误，其余字段保留。坏项目任务的 merge 投影降级 null，防止这个间接配置读取再次中断列表；排队 MergeGate 的展示查询也仅容忍 ConfigError，执行调度仍使用原严格入口。
- CLI status 显示项目名与「配置有误：原文」，健康任务仍显示。同步 technical-design RPC 说明。
- 开发中修正测试夹具：macOS 临时路径的 /var 与 /private/var 别名应以注册规范路径比对错误原文；新项目默认可能没有配置文件；CLI 添加目录要求先存在；测试插入点从 dispatcher 匹配分支移到既有测试模块。这些均非上一轮审计缺陷。

## 命令、计数与基线

- 原样验收：`cargo test -p automed --lib -- probe_rejection_ config_get_ dashboard_tolerates_ && cargo test -p autome-cli dashboard_tolerates_`。日志 `.autome/output/m14-final-lib.txt` 与 `m14-final-cli.txt`，退出 0，内核 6 + CLI 1 = **7 个用例全部通过**，失败/忽略均 0。CLI 单测目标无匹配项，集成目标实际运行 1 个，不将空目标计为用例。
- 同过滤器开发基线为既有 config_get 1 个（`.autome/output/m14-baseline.txt`）；新增内核 5、CLI 1，因此从 1 增至 7，**+6**。上一轮 M-13 验收总数为 30，本轮为 7，**−23** 来自切换里程碑过滤范围，不是删除测试；本轮未重跑 M-13 的事件、会话、Web 验收。
- 直接相关子集：`cargo test -p automed --lib -- gate_queue_ project_ a_nested_directory_is_refused a_directory_of_repositories_is_probed removing_a_project`，退出 0，69 个通过，日志 `.autome/output/m14-related.txt`。覆盖项目添加/移除、原配置/费用投影、双车道和调度坏配置隔离。
- serde 兼容：`cargo test -p autome-domain before_merge_defaults_override_and_json_compatibility`，退出 0，1 个通过，其余目标无匹配，日志 `.autome/output/m14-serde.txt`。
- 上述正向验收与相关子集合计执行 77 次，按用例名称去重为 **73 个**（有 4 个重叠），全部通过。重复执行与负向对照不累计为新增用例。
- 仅对改动 Rust 文件执行 rustfmt 格式化；git diff --check 通过。遵从本轮指令，未运行 cargo fmt --all -- --check、clippy、Web 快检及 ci-local 全量门禁，交宿主运行。

## 设计指定负向对照

在同一次临时注入中：probe 过滤掉 contains_project；dashboard 恢复 project_warnings 的 `?` 传播；config.get 删除三项缺省补齐。原样内核验收 6 个中 3 个失败（对应 probe_rejection_matches_project_add_for_each_kind、dashboard_tolerates_a_broken_project_config、config_get_reports_every_resolved_key_and_marks_defaults），退出 101，日志 `.autome/output/m14-negative.txt`。注入已通过 ensure 恢复；恢复后完整验收 7 个全绿。没有保留注入代码。

## 标待审之前的自审清单

1. **情形表逐行：已执行。** 四种拒绝分别对同一目录调 probe/add，核对 kind、message、project_id；正常目录与未创建目录为 null 且 probe 不创建；移除后可重新添加且复用 ID。空配置核对 14 键、defaults 及顶层键基线；显式等于缺省值不误标，项目并发受全局限制。坏/好两项目并存时 dashboard 成功，坏项目运行中、排队、待处理任务均保留，健康任务投影完整；project.get 其余键保留；CLI 实际进程输出错误原文及健康任务。
2. **设计点名失败分支：已执行。** 拒绝：文件/重复/内嵌/包含已有项目四类；配置失败：非法 TOML、gate.concurrency=0 的内核校验原文、配置路径为目录的真实读盘失败；空配置与缺失配置恢复可读。超时、外部服务不可达、授权拒绝不适用：本里程碑没有外部服务、会话或新增授权操作。
3. **全部迁移：已执行。** 配置有效→无效→恢复在 dashboard/project.get 各验证，恢复后 config 与 merge 投影重新有效；项目 active→removed→重新添加验证同 ID。没有新增状态机，展示覆盖 running/queued/waiting 三个已有分组；调度仍严格的既有相关子集通过。
4. **输入域边界：已执行。** 中文/空格目录、未创建的多层子目录、符号链接别名、missing/.. 归一化、单项目/两项目、空配置/缺失配置、显式 false、each_round、并发 0 拒绝、1 与 10 边界及全局封顶。日期、前导零数值串、闰日不适用：接口不接收这些输入类型。

## 结论与边界

四项自审已执行，M-14 申请待审，不标已完成。只改当前 worktree，不推送、不启动下一会话；测试使用隔离账本和临时项目。宿主快检与全量门禁尚未运行，不以本轮子集替代。提交到当前分支；若沙箱拒绝则记录「提交由 Autome 兜底」。
