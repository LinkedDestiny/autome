# M-04 · 实现轮 5/25

## 范围与实现

- 本轮仅推进 M-04（D5 渲染、D7）：共用 `task_json.merge` 投影，CLI 列表短提示、status/detail 完整提示、配置警告，以及合并队列事件渲染。
- `behind_count` 使用 `branch..base`；仅 Active AwaitMerge 读取落后数量，多仓库按成员默认分支求和；任一 git 读取失败记录 warning 并降级 `stale: null`。终结任务 `merge: null`。
- 设计中的 `task.list` 不是现有 RPC：实际 `Client::all_tasks` 调用 `project.get` 获取任务列表。本轮按真实接口实现与测试，并就地纠正 D7，不新增 RPC。
- 上轮审计 M-03 通过，没有被退回的实现缺陷，也没有必须逐字搬入的审计检查。
- 本地主线只读核对：T-51 已在 `bf399dbf` 合入；`git log main --grep='T-53'` 未命中，不 fetch。队列排序口径未改。

## 命令与结果

- 验收契约：`cargo test -p automed --lib merge_view_ && cargo test -p autome-cli merge_view_ && cargo test -p automed --test end_to_end merge_view_`。
- 验收用例总数 **6**（lib 3、CLI 2、e2e 1）；相对上一轮 M-03 验收基线 6 **差 0**，本轮从冲突用例切换为显示用例，没有删除旧测试。相对本轮起点相同 `merge_view_` 筛选口径 0 项 **+6**，均为新增。
- 直接相关回归：`cargo test -p autome-cli render::`，31 项通过（含本轮 2 项 CLI 验收，既有事件渲染 29 项）。日志 `.autome/output/m04-render.txt`。
- 去重后本轮共 **35 个用例**；相对上一轮 22 个 **+13**：验收 6→6，额外相关回归 16→29。重复跑、负向对照和空筛选程序不累计。
- 最终验收日志：`.autome/output/m04-accept-lib.txt`、`m04-accept-cli.txt`、`m04-accept-e2e.txt`；整链退出码 0，6 项全部通过、0 失败、0 忽略。
- 开发中新增列表覆盖曾报 `UnknownMethod: task.list`；修正为真实 `project.get` 后通过（见上文接口修正）。原日志 `.autome/output/m04-lib-final.txt` 后续成功运行已覆盖，该失败原因明确记录在此。
- 未运行 `sh scripts/ci-local.sh` 或宿主快检组合；fmt/clippy/lint/typecheck 由宿主按本轮指令执行。本轮只执行格式化与直接相关测试。

## 负向对照

1. 临时将 `behind_count` 范围改成 `base..branch`，执行 `cargo test -p automed --lib merge_view_behind_count_counts_commits_only_on_the_base`：退出 101，1 项失败；日志 `.autome/output/m04-negative-count.txt`。恢复生产代码后纳入最终绿验收。
2. 临时将 stale 条件改成 `true || task.state == Active AwaitMerge`，执行 `cargo test -p automed --lib merge_view_task_get_reports_a_stale_await_merge`：退出 101，1 项失败；日志 `.autome/output/m04-negative-state.txt`。恢复生产代码后纳入最终绿验收。
3. 两项注入均已还原，不提交临时验证产物。

## 标待审前自审清单

### 1. 情形表逐行

- D5 出队 rebase：CLI 覆盖单仓库与多仓库，使用 `base_sha` 短值，断言不误用 head。
- D5 旧 rebase / behind 授权：既有 render 回归继续通过；用户授权与撤销各有断言，不混为旧的落后回退。
- D5 locked / unlocked / waiting：拿锁、释放和 MergeGate/Merging 两种等待文案均检查。
- D5 direct：user/project/behind 三种来源逐一检查。
- D7 gated / 非 gated：完整文案、列表短文案、授权、等待均检查；无门禁需求为 false，Bug 即使没配项目门禁仍为 true。
- D7 读接口：task.get、project.get（ls 真实数据源）、dashboard.get 均经实际派发调用检查；dashboard 同时检查持锁者、等待者、授权来源、空宿主队列位置和配置警告。
- D7 多仓库：main 落后 1、trunk 落后 2，总计 3，逐仓库明细检查。

### 2. 失败分支

- git 读取失败：缺失分支；多仓库中一个仓库路径失效；两者 task.get 均成功、stale 降级 null。
- 空/无效 git ref：behind_count 返回错误，不伪造零落后。
- 拒绝授权、外部服务不可达：不适用，本轮是本地只读投影和渲染，不修改授权写接口、不访问外部服务。
- 超时：不适用，本轮未新增超时机制，沿用 git 有界执行；设计 D7 点名的 git 失败降级已通过上述错误检查。

### 3. 全部迁移

- 本轮不新增状态转换；读视图对 MergeGate→AwaitMerge→Implement→Merging 的状态切换逐项验证，只有 AwaitMerge 显示 stale；Cancelled 返回 merge null。
- AwaitMerge 落后→rebase 后零落后：stale 非空→null；再前进→非空；user→behind→撤销：来源逐项更新；非 MergeGate/Merging 节点不显示等待。
- 真实 e2e：A、B 均已通过门禁停住，A 合并推进 main，B 的 stale.behind 与独立 `git rev-list --count` 一致；对 B 按合并后 rebase、恰好多 1 次 MergeGate 作业并到 done。
- 事件输出两种等待方向、三种授权来源、锁进入/退出、授权/撤销均已逐项检查；JSON 事件路径未改。

### 4. 输入域边界

- 覆盖落后 0、1、多个提交；单仓库空名称、多仓库名称、不同默认分支；缺省 merge 字段、空 stale、撤销后的空授权来源。
- 日期/闰日/跨年/前导零不适用：输入是 Git 提交计数与结构化 JSON，不接收日期或用户数字文本。
- u32 最大提交数未构建 42 亿提交夹具；git 输出采用 fallible `parse::<u32>`，溢出走错误降级。汇总用 u64，避免多个 u32 计数相加溢出。

## 限制

- H-01 已安装 CLI 的人工排版确认、H-02 Web/桌面真实 UI 验收仍由人工清单跟踪，本轮未取证。

## 收尾

- `git diff --check` 退出 0；四项自审已完成后将 M-04 标为待审，状态块 implementation-round 为 5/25，current-milestone 仍为 M-04。
