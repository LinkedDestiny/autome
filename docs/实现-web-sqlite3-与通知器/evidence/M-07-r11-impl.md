# 实现 #11：宿主 End to end 门禁复验

## 范围与结论

起始 HEAD `8b78832`，工作区干净。本轮按用户要求只处理门禁，不推进里程碑；M-01..M-07 原有已完成状态、reopen 与审计结论保持不变，implementation-round 写为 **11/35**。沿用 M-07 的门禁复验记录归档，不表示修改了 M-07 产品行为。

未取得完整门禁通过证据，未宣称修复成功。宿主给出的尾部只包含 End to end 步骤失败，没有失败用例和断言；本次同命令下 End to end **80/80 通过**。上一轮 B-05 的两个 30 秒工作区用例也通过，因此不能确定宿主此次与 B-05 同因，更不能凭步骤名修改超时。

任务 §4.1 明确禁止修改 crates/，本轮没有将一般的门禁修复要求解释成取消该明确边界。实测失败位于 crates/ 或沙箱监听权限，没有 Web 产品代码修复；只提交本轮证据和当前状态指针。上一轮审计 #8 没有发现缺陷或提供新检查，故无审计检查待迁入；本次失败检查原本就在项目测试体系，无需重复复制。

## 命令与结果

1. `sh scripts/ci-local.sh > .autome/output/r11-ci-before.txt 2>&1`：退出 **1**，548 秒。全过程未修改源码，属于同一代码版本的完整复验。Format、Clippy、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 通过；Test、Web test 失败。
2. Test 阶段失败用例：`gate::tests::requeued_discard_waits_for_the_wrapper_pid_before_removing_files`，`crates/automed/src/gate.rs:380`，断言原文 `old wrapper must be dead before paths can be reused`。该用例在 `discard_node` 返回后立刻 `child.try_wait()`，结果仍未退出。检查调用链：`discard_node` → `kill_group`，后者 SIGTERM 后等 500ms，再在存活时发 SIGKILL 并立即返回，没有等待 SIGKILL 后退出；这是可解释并发时失败、单跑通过的时序窗口，尚未通过额外插桩证明本次必走该分支，不把推断写成已确认根因。不能以延长测试截止或吞断言消除这个信号。
3. 原样复验现有用例：`cargo test -p automed --lib gate::tests::requeued_discard_waits_for_the_wrapper_pid_before_removing_files -- --exact --nocapture > .autome/output/r11-gate-recheck.txt 2>&1`：退出 **0**，1/1 通过，609 未选，4.23 秒。源码未变，这不是修复后的红→绿证据，只说明单跑未复现。
4. Web 失败为 `listen EPERM: operation not permitted 127.0.0.1`，伴随 `File watcher error: EMFILE: too many open files, watch`；涉及 gateway-http、eventbus-live、notifier-live、server。未跳过或删改任何测试，当前权限禁止提权。真实账本 notifier-real 两例通过。

原始日志与汇总索引 `.autome/output/r11-results.txt` 留在允许的 output 目录，不转存进任务文档。没有以更换命令、设置串行测试环境变量或删断言制造通过结果；未修代码且已确认监听阻塞，不重复整条门禁以尝试撞出绿色。

## 用例总数与基线差

- Web：33 文件、**301 例**，271 通过、11 失败、19 未执行；较上一轮及宿主 301 **差 0**，本轮没有增删用例，未执行不算通过。
- Desktop：**104 例**，103 通过、1 个基线已有跳过；较上一轮 104 **差 0**。
- Rust workspace：实际执行 **987 例**（21+11+345+610），986 通过、1 失败。较上一轮实际 1067 **少 80**：本次在 automed lib 失败后 Cargo 停止，未执行其后的 end_to_end；较完整基线 1090 少 103（80+16+7），不是删除测试。
- 独立 End to end：**80 例全部通过**，较上一轮 80 **差 0**。该步补跑了 workspace 未执行的 80 例，但不能补足后面 16+7 例。
- Adapter flags：1 例通过、6 未选，较上一轮差 0。额外单测复验只选原有 1 例，无新增用例，不计入全量总数。

## 四项自审清单

1. **情形表逐行**：本轮无产品修改、无新里程碑报审，新增情形表不适用。核对门禁每一步，逐步结果如上；End to end 80 例和 Web 29 文件通过不替代 4 个监听测试文件的缺失证据。
2. **失败分支**：已执行完整门禁，保留 wrapper 未退出断言与监听权限失败；单跑原断言通过。新增授权、读盘、外部服务分支不适用：未增加任何运行时分支，也未过滤原失败。
3. **全部迁移**：产品状态机迁移不适用，无代码变化；核对设计 status、current-milestone、reopens、convergence-mode 和完整里程碑表保持原样，仅更新实现轮次和 next-action。
4. **输入域边界**：不适用，本轮仅复验与文档记录，没有新增或修改输入域；没有通过缩窄已有测试输入减少覆盖。

## 后续与收尾

Autome 在提交后从本 worktree 重跑 `sh scripts/ci-local.sh`，以该提交的宿主结果为准。若 End to end 再失败，须保留失败用例全名、断言及上下文；如果 wrapper 失败复现，交内核维护范围处理，保留项目原有用例作为回归。设计 B-06 记录本次范围外失败，D-01 保留宿主命令。没有将环境或范围阻塞标为协议失败。

本轮仅更新设计状态/证据指针、Backlog、追加一行 retro 与新增本证据；不启动下个会话、不切分支、不推送。提交结果在会话最终回复说明。
