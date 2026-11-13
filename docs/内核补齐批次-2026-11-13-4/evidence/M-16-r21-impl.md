# 实现 #21：合并门禁 e2e 修复

## 范围与依据

本轮是集成修复轮，implementation-round 为 21/80，不推进新里程碑，不修改已关闭里程碑表与最新证据指针。上一轮审计通过，无待搬迁的审计检查；宿主发现的两条检查本就在项目测试体系中，先原样复跑。

宿主原始日志：主检出 `.autome/output/gates/T-95/merge_gate-842714cc3a67.log`，e2e 共 258 个，256 通过、2 失败。索引保存于本 worktree `.autome/output/r21-gate-index.txt`。失败分别为 `the_merge_panel_offers_the_merge_when_only_main_has_moved` 与 `legacy_pointer_e2e_restore_failure_keeps_todays_failure`。

## 根因与修复

1. M-13 / F-R3：旧合并面板测试最后仍断言合并清理后 `available=false`，与设计要求使用合并提交继续展示改动冲突。改为断言 `available=true`、`merged=true`、文件清单非空；此前主分支移动、脏文档拒绝、自动 rebase、最终完成的断言全部保留。全文件检索仅此一处旧式单行断言；M-13 的缺提交、单父提交负向断言仍应为 false，保留且复跑。
2. M-10 / J-T1：`resume_setup` 在返回前已经批准并启动第 5 个会话，随后夹具提交 v7 模板，与该会话的 `git add -A docs` / commit 共享索引。具体交错：夹具已暂存模板并确认非空，会话先提交该模板，夹具再 commit 即退出 1（stderr 为空），符合宿主失败位置。故障锁在第 8 会话才创建，不是第 5 会话初始化失败原因。现改为在 `await_design_approval` 停稳时冻结模板及版本，断言无运行会话，然后批准。v7/v8/还原失败共用此夹具，原有指针还原、集成审计、锁失败与保留文档断言未弱化。

没有产品代码或协议正文变化，没有增加测试函数。为了确定性防回归，在模板写入前新增“无运行会话”的时序断言，已验证修复前红、修复后绿。

## 命令与结果

所有原始输出在 `.autome/output/`；测试使用项目已有临时仓库/账本夹具。

- 原样复现及修后同命令：`cargo test -p automed --test end_to_end -- legacy_pointer_e2e_restore_failure_keeps_todays_failure the_merge_panel_offers_the_merge_when_only_main_has_moved`。修前 2 个：1 过 1 败（r21-red.txt，稳定失败为旧 available 断言）；修后 2 个全过（r21-green.txt）。
- 原锁失败用例额外单跑 4 次均通过（r21-race-1.txt 至 r21-race-4.txt），未宣称本地复现随机 Git 错误。加入时序断言后、修复前运行 `cargo test -p automed --test end_to_end legacy_pointer_e2e_restore_failure_keeps_todays_failure`，1 个失败，原因为冻结时已有运行会话（r21-order-red.txt；r21-race-5.txt 也是加入断言后的结果）。修后同用例由上述原命令及相关子集再次通过。
- 相关子集及 M-10 automed 验收超集：`cargo test -p automed -- closed_pointer_ legacy_pointer_ the_merge_panel_ task_changes_status_`，库 12 + e2e 7 = 19 个全过（r21-related.txt）。
- M-10 其余验收：`cargo test -p autome-domain protocol_ref_version`，1 个通过（r21-domain.txt）；`cargo test -p autome-cli legacy_pointer_`，1 个通过（r21-cli.txt）。
- 修改文件的 `rustfmt --edition 2024 --check crates/automed/tests/end_to_end.rs` 与 `git diff --check` 均退出 0（r21-format.txt 为空）。未跑项目快检或全量门禁。

最终去重用例总数 **21，21 通过、0 失败、0 跳过**。相比实现 #20 的 106 个为 **-85**，因为本轮只跑指针还原与改动展示相关范围，未重跑上一轮协议/调度/迁移范围；不是删减测试。相同选择范围用例数变化 **0**。红绿及重复运行不重复计数；e2e 注册总数仍为 258。

## 四项自审

1. **情形表逐行**：J 的 v7 自动还原并降级提醒、v8 自动还原并点名规则、还原失败保持失败，三条 e2e 全过；F 的合并前/后、单仓库/工作区清理后读取、增删改重命名均由 task_changes_status 子集通过；合并面板的主分支未移动/已移动/脏文档/清理后四阶段均通过。
2. **失败分支**：指针还原读盘与提交失败、索引锁、缺基线、缺 revision、缺合并提交/单父提交、脏工作树拒绝均已通过。授权、超时、外部服务不可达不适用：本轮未改这些路径，也无外部服务。
3. **全部迁移**：审批等待→冻结旧协议→批准启动；实现改指针→还原→集成审计→合并等待，以及还原失败→任务失败；合并等待→主分支移动→自动 rebase→完成→归档改动可读，相关 e2e 全过。不增加产品状态迁移。
4. **输入域边界**：v7/v8、无基线、围栏与开放里程碑不误改、空/二进制 diff、特殊路径、缺 revision、缺/无效合并提交均通过。日期、前导零、数值最大值不适用：本轮没有这类输入变化。

## 交接

保持既有里程碑状态；交由独立集成复验及宿主快检、合并门禁判断。H-01 付费行为验收仍待人工确认。本轮不启动下一会话。
