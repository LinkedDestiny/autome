# 实现 #11：rebase 集成修复（关联 M-01）

## 范围与状态

简报和里程碑表均确认六项已由审计关闭，没有开放里程碑。本轮执行用户追加的 rebase 指令，不新增里程碑、不重新关闭既有项。状态保留已完成，implementation-round 为 11/30，current-milestone 与 next-action 为无。上一审计通过，没有要求逐字搬入的缺陷检查。

`origin/HEAD` 指向 main；本地 main 为 `02a05496`。`git rebase main` 在 store.rs 重现冲突，解决后 `GIT_EDITOR=true git rebase --continue` 完成全部 52 步；`git merge-base --is-ancestor main HEAD` 返回 0。没有 merge、切分支、推送或启动下一会话。日志：`.autome/output/r11-rebase.txt`、`r11-rebase-continue.txt`。

## 根因、修复与同类检查

1. main 的 v6 已加入 `sessions.limit_minutes`，本任务原 v6 创建 `task_after`，两者不能共用版本。原样保留 main v6 事务，依赖表独立顺延到 v7，SCHEMA_VERSION 与版本断言同步更新。已有 v6 库会运行 v7，新库按顺序运行两段迁移。
2. `deps_store_migration_preserves_queue` 改为构造真实 v6 起点（删除依赖表并设版本 6），保留队列与空依赖断言，增加现存会话 limit_minutes=42 升级后不变的断言。main 的 v5→v6 时限迁移测试、既有 v4 升级测试均保留。
3. rebase 的文本自动合并漏掉一个签名适配：main 的 session_json 已需要 ctx、project、session，而本任务 dashboard 排队组仍传一参数函数。改成与 running/waiting 一致的闭包调用。检索全部 session_json 调用点，仅这一处落后；编译及相关 dashboard/时限展示用例通过。
4. 当前设计和技术设计的依赖迁移版本统一为 v7，历史证据只读不改。原方案的 v6 不能继续用于已发布默认分支；本轮是集成适配，不改依赖产品规则。

## 命令、结果与数量

- `cargo test -p autome-domain deps::`：12 个通过，日志 `.autome/output/r11-domain.txt`。
- 初次 `cargo test -p automed --lib store::`：编译失败 E0593，定位 dashboard 排队组旧 session_json 签名，尚未执行用例。日志 `.autome/output/r11-store.txt`。修复后原命令 58 个全部通过，日志 `.autome/output/r11-store-green.txt`。
- `cargo test -p automed --lib after_blocked_dashboard_preserves_project_queue_order`：1 个通过，日志 `.autome/output/r11-dashboard.txt`。
- `cargo test -p automed --lib session_timeout`：4 个通过，其中 1 个与 store 子集重复。日志 `.autome/output/r11-timeout.txt`。
- `cargo test -p automed --lib timeout_visible_`：3 个通过，日志 `.autome/output/r11-timeout-visible.txt`。
- `git diff --check`：通过。未运行全量门禁、cargo fmt/clippy 或 Web lint/typecheck 宿主快检。

去重用例总数为 **77（12+58+1+4-1+3）**，全部通过，0 失败、0 跳过。相对实现 #10 的 95 个少 18 个，来自本轮改跑 store/依赖/时限集成子集，不代表删除测试。M-01 验收自身为 70 个，相对审计 #1 的 69 个增加 1 个，来自 main 新增的 session_timeout_store_migration_limit_and_pid；本轮没有新增测试函数，仅增强迁移断言。main 的 store 源码基线 53，加本任务 5 为 58。编译失败与重复执行不重复计数。

## 标待审前自审四项（本轮保持既有审计关闭）

1. **情形表逐行**：M-01 的 12 条 deps 检查全部通过：自引用、不存在、跨项目、直接环、三节点环、输入上限、去重保序、中文单行拒绝、需求可改状态表、Bug 可改状态表、阻塞/满足判断、三种移动及拒绝。store 五条依次检查迁移后空依赖和顺序不变、创建外键失败整体回滚、替换清空及实现历史、项目隔离重排、阻塞通知往返，全部通过。额外检查会话时限迁移与读取，避免冲突修复覆盖 main 功能。
2. **失败分支**：不存在/跨项目/自引用/环/超过 8 项均拒绝；外键失败不留任务或简报；不可改状态拒绝；自己或不存在参照拒绝；新版或非本应用账本拒绝，由 deps/store 用例覆盖。外部授权、网络不可达不适用：本轮是本地 SQLite 迁移与序列化签名适配。没有新增读盘超时处理；现有 store 拒绝/失败用例在该子集内通过。
3. **全部迁移**：新库→最新、v4→最新、v5→最新、v6→v7、最新重复 migrate 由 store 子集通过；v6 会话时限 42 保留、v5 旧会话时限为空、NULL 队列位置保留。依赖空→新增→替换→清空、通知未设置→设置→清空、无 Impl 历史→有 Impl 历史由现有五条依赖 store 用例通过。产品调度状态机未修改；可编辑状态全集由 domain 状态表检查。
4. **域边界**：依赖空数组、单项、重复项、8/9 项（含重复 9 项）、T-02 前导零均通过；队列 NULL 与已有序号、跨项目隔离通过；会话时限 None、42、u32::MAX 以及展示边界由 store/timeout_visible 子集通过。日期闰日、跨年不适用：本轮迁移不解析日期输入。

测试全部沿用项目临时账本；未用构建二进制打开真实账本。宿主仍需执行快检与合并门禁；H-01/H-02 人工验收保留。
