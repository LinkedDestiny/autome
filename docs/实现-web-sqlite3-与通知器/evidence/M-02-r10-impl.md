# 实现 #10：真实账本测试超时修复

## 范围与结论

本轮为门禁修复轮，不推进新里程碑。起始 HEAD `55ad98d`，工作区干净；M-01..M-07 已由审计关闭，全部状态及 reopen 原样保留，implementation-round 为 **10/35**。证据归于拥有真实账本测试的 M-02。上一轮审计 #7 通过，没有待迁入的审计失败检查；宿主失败检查本来就在 `apps/web/test/notifier-real.test.ts`，本轮原样执行，保留全部断言，不新增重复用例。

## 红 → 定位 → 修复

1. 原样执行 `npm test --prefix apps/web -- test/notifier-real.test.ts > .autome/output/r10-real-red.txt 2>&1`，退出 **1**：2 个用例均失败，第一例在 **90232ms** 时造账本命令失败；第二例 `expect(ids).toHaveLength(14)` 实际为 `[]`。afterAll 同时出现 `ENOTEMPTY`，与宿主给出的第二例行号和三段错误结构一致。
2. 未修改夹具的测量命令：`bash apps/web/test/fixtures/w4-ledger.sh "$PWD/.autome/output/r10-ledger-timing" "$PWD" > .autome/output/r10-ledger-timing.txt 2>&1`，退出 **0**。从 `output/project-add.json` 创建到最后 `output/scratch-root.txt` 写入约 **139.08 秒**，完整输出 163 条账本事件。该测量与 Rust 门禁并行，说明有负载时合法执行会超过 90 秒；不是通知断言失败，也不是脚本死循环。
3. 原因：第一例在 `await run()` 里被 `execFile` 的整段 90000ms 超时终止，还没有执行 `notifier.pull()`；因此通知库为空，第二例连带失败。脚本被中途杀死后子进程尚在写临时目录，删除目录又触发 `ENOTEMPTY`。这三处是同一次中断的连锁结果。
4. 修复仅在该测试文件：完整七任务夹具限时 **300000ms**，第一例限时 **310000ms**，后 10 秒供通知拉取与断言；extra 单任务夹具仍为 90000ms，第二例仍为 100000ms。没有改产品代码、夹具行为、通知数量、测试断言、重试策略、全局测试配置或门禁脚本。仍有有限截止时间，没有跳过或吞掉失败。任意真正超时后的通用进程树回收不是本次修复的内容。

原始输出全部留在 `.autome/output/`；测量目录中的测试仓库仅为临时夹具，不是本任务的新 worktree。

## 四项自审清单

1. **情形表逐行**：执行 M-02 验收命令（见下文），七条规则分别覆盖 await_design_approval、await_merge、await_info、failed、project_alert、done、not_a_bug，均通过；表外状态/事件不通知通过。真实账本七类计数仍为 3/2/2/3/2/1/1（按设计计数顺序），14 条通知全部补齐项目，重启只新增第 15 条，通过。未更改既有里程碑验收契约。
2. **失败分支**：既有测试中的补齐离线、超时（无 code）停游标并重试；带 code 拒绝仍生成降级通知；注入游标写盘失败整事务回滚，全部通过。九种 FailureReason、未知原因与未知告警渲染通过。新增产品授权、外部服务或读盘分支不适用：本轮只调整测试期限。原 90 秒超时红例及保留断言的绿例另列，不拿空库当作产品缺陷。
3. **全部迁移**：首次空账本/已有历史初始化；运行→重启不重复；停服→新增→补齐；事务失败→恢复；人工通知已写/游标回退→去重；换账本→清缓存；同状态重复/离开再进入；并发 pull 合并；close 与在途补齐关闭，均由原 25 例执行通过。新增状态迁移不适用：本轮无产品状态机变化，既有里程碑状态不变。
4. **输入域边界**：既有空账本、单事件、分页 pageSize=2 跨页、MAX_SAFE_INTEGER 首次查询、无 task_id 告警、失败详情长度 0/1/60/61、未知 kind 均通过。前导零、闰日、跨年不适用：本轮没有新增日期或字符串数值输入域。

## 验收命令与总数

- `npm test --prefix apps/web -- test/notifier-rules.test.ts test/notifier.test.ts test/notifier-real.test.ts > .autome/output/r10-notifier-green.txt 2>&1`：退出 **0**，3 文件、**25/25** 通过、0 跳过，149.04 秒。较 M-02 原基线 25 **差 0**；本轮只改超时，不增删用例。
- 与红例完全相同的命令：`npm test --prefix apps/web -- test/notifier-real.test.ts > .autome/output/r10-real-green.txt 2>&1`：退出 **0**，**2/2** 通过、0 跳过，107.18 秒；较红例及原基线 2 **差 0**。afterAll 正常完成，无 ENOTEMPTY。此处是同一项目回归检查的红→绿，不是用新断言替换原检查。

## 完整门禁的运行边界

`sh scripts/ci-local.sh > .autome/output/r10-ci-red.txt 2>&1` 在改动前启动。等待 Rust 阶段期间，先用独立的原始真实账本命令取得上面的红证据，再修改超时；完整门禁到达 Web lint/typecheck/test 时已读取修复后的文件。因此这个文件名虽为 red，**不是整条未修改版本的基线日志**，也不能用它声称整条门禁红→绿。通知回归有独立的、修复前后同命令证据。

范围外 Rust 失败已记设计文档 B-05：workspace 全量阶段的 `bug_loop_a_workspace_bug_checks_out_every_member_to_locate_then_keeps_only_the_named_repos`（end_to_end.rs:4531），以及独立 End to end 阶段的 `bug_regression_guard_checks_each_workspace_member`（:5069），都在 `assert!(std::time::Instant::now() < deadline, "{}", w.why(&id));` 报错，实际为 `Active { node: MergeGate }`。源码中的 30 秒整体截止先于流程到达等待用户态；不能把它等同于通知器故障，也未证明内核产品行为错误。按任务硬约束不修改 crates/。

完整门禁最终退出 **1**，880 秒（结果索引 `.autome/output/r10-results.txt`）：

- Format、Clippy、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 通过。
- Web **33 文件、301 例**，271 通过、11 失败、19 跳过；较宿主及上一轮 301 **差 0**，没有增删用例。`notifier-real.test.ts` 属于通过的 29 文件，原两例失败不再出现。失败文件为 gateway-http、eventbus-live、notifier-live、server；`listen EPERM: operation not permitted 127.0.0.1` / `File watcher error: EMFILE: too many open files, watch` 阻止监听和初始化，19 个未执行用例不计为通过。此权限限制不能在当前会话提权解除。
- Desktop **104 例**，103 通过、1 个原有跳过；较上一轮 104 **差 0**。
- Rust workspace 本次实际执行 **1067 例**（21+11+345+610+80），1066 通过、1 失败；较上一轮 1090 **少 23**。差值来自 Cargo 在 end_to_end 失败后停止，后面的 16 例与 7 例测试二进制未执行，并非删测试或被统计为 skipped。独立 End to end 再执行 **80 例**（较上轮 80 差 0），79 通过、1 失败，具体两处截止失败见上文及 B-05。Adapter flags 另选 1 例通过、6 例未选，不能把它当成补跑整组。

没有取得整条门禁通过证据，不宣称任务重新验收完成；真实账本回归修复已取得同命令红→绿和 M-02 25 例通过。Autome 应在本轮提交上重新执行 `sh scripts/ci-local.sh`，以提交后的宿主结果为准。设计 D-01 保留宿主命令和失败证据要求。

## 收尾

`git diff --check` 通过。额外校验：implementation-round 为 10/35；status、design-round、current-milestone、reopen、convergence-mode、repos（不存在）与 HEAD 相同；里程碑表逐字相同；retro 只追加一条五段、不超过 200 字记录。无负向变异遗留。未改 `.autome/` 配置或规则，只写允许的 output；未切分支、推送或启动下一个会话。
