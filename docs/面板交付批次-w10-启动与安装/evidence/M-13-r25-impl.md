# 实现 #25：宿主快检退回修复

## 范围与状态

遵循本轮最后指令，只修复 d4afc4513d 的五项快检失败，不推进新里程碑。开场 M-01…M-13 均已完成、审计 #22 通过；所有里程碑状态和最新审计证据指针原样保留。沿用 M-13 归档集成证据，implementation-round 更新为 25/65。未修改产品实现、协议或内核，未启动下一会话，未运行全量门禁。

## 复现与修复

宿主列出的五个检查原本就在项目测试体系，不存在需要另搬入的临时审计脚本。修改前原样运行五个文件：

`cd apps/web && npx vitest run test/env-page.test.ts test/live-status.test.ts test/settings-onboarding.test.ts test/shell-data.test.ts test/web-launcher.test.ts`

- 红：`.autome/output/r25-red.txt`，38 条，23 通过、15 失败、0 跳过。前四个文件各失败一条，与宿主报错相同。web-launcher 的 11 条真实进程用例在预检 `spawnSync ps EPERM` 失败，尚未启动服务；本地未复现宿主的进程数 2，不能把权限错误当作同一缺陷的复现。
- 绿/环境阻塞：相同命令输出 `.autome/output/r25-green.txt`，仍为 38 条，27 通过、11 失败、0 跳过。四项确定性缺陷恢复通过；11 条仍在 ps 预检失败，全部需要宿主复验，包括并发启动。没有条件跳过、没有替代进程清理断言。

具体根因与修改：

1. env-page：设计 B2 已将 revalidateAll 合并到 1000 ms 窗口，旧测试却在发布消息后立即要求调用一次。改为假时钟，999 ms 无调用、1000 ms 恰一次；无关消息等待完整窗口仍无额外调用；env.probing 仍执行真实路由匹配判断。
2. settings-onboarding：旧 vi.waitFor 默认 1000 ms，恰等于合并窗口，在截止边界竞态失败。改为假时钟的 999/1000 ms 断言；hello 和退订后的 onboarding.ended 各等待完整窗口，确认不刷新。finally 释放订阅、排空定时器并恢复真实时钟。
3. live-status：路由在合并计时器触发前仍是 idle，旧等待立即返回，loader 调用数仍为 1。真实 hook 和路由保留，渲染后推进 999/1000 ms，再等待路由完成。覆盖 initialCore=offline/connected × succeeds=true/false；首次恢复一次、持续失败不循环、再次离线恢复只在需要时重试。
4. shell-data：设计 B3 的 root loader 应附 clock，旧精确等值断言仍只期望 shell。固定系统时间，保持精确 toEqual，逐项核对原 shell 加 clock.now 和宿主 IANA 时区，不放宽成部分匹配。
5. web-launcher：设计 A5 明确允许两个 start 分别 spawn serve，竞争在 serve 内解决。ensure 每 100 ms 观察到活登记即可返回；竞争者在 acquireLaunchLock 中每 200 ms 重试，父进程完成时落败 serve 可能尚未退出。旧 h.servers() 按进程命令计数，把这个暂态也算第二个服务。改为至多 3000 ms 的条件轮询，要求进程 PID 列表精确等于登记 PID，再判健康为 live；没有改计数函数、没有隐藏竞争者、没有固定 sleep。两个持久服务仍会令断言失败。此因果解释来自源码，真实收敛与负向注入尚未实跑，必须由宿主确认；不宣称已排除所有真实并发缺陷。

同类检查范围：搜索全部测试的 subscribeAdminRevalidate、revalidateAll、root loader 等值断言，并检查 live-routes、board-live-timing、load-context。前两者已有合并窗口测试，load-context 在 r24 已补 clock；本轮不重复修改。直接 serve/start 并发用例本已使用条件轮询。

## 其它命令、总数与基线

- `cd apps/web && npx vitest run test/env-page.test.ts test/live-status.test.ts test/settings-onboarding.test.ts test/shell-data.test.ts test/live-routes.test.ts test/clock.test.ts`：6 文件、45 条全部通过、0 跳过，见 `.autome/output/r25-green-unit.txt`。相对上述四个失败单测文件 26 条，增加 19 条来自 live-routes 11 和 clock 8，不是新增测试。
- `cd apps/web && npx vitest run test/web-instance.test.ts -t '原子排他|只删自己的|pid 已死|端口无监听|端口换主人|锁再旧|死锁持有者|三态判定'`：收集 14 条，执行 8 条全过，6 条因显式名称过滤未执行（runner 显示 skipped），见 `.autome/output/r25-lock.txt`。这不是完整启动验收，不把过滤的六条报作通过。
- 原样执行本轮指定快检：`if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi`，退出 0、16 秒，五步通过：Format、Clippy、Web install、Web lint、Web typecheck。输出 `.autome/output/r25-quick.txt` 明确写着“未提供本轮起点，未跑改动相关的测试”。因此本地快检只证明这五步；最终以 Autome 提交后带起点的重跑为准。
- `git diff --check` 通过。原始输出仅放 `.autome/output/`。

用例数量：五文件红/绿均为 38，差 0；本轮无新增或删除 test/it，宿主上一轮 131 文件、987 条的集合预期仍为 987（差 0，未本地重跑全套）。与 r24 的八文件 76 条相比，本轮五文件 38 条差 -38，仅因修复目标选集不同，不是删除用例。合并去重后本轮涉及 8 个文件、收集 71 条，实际执行 65 条，其中 54 通过、11 被 ps 权限阻塞，6 条被名称过滤；重复执行不重复计数。

## 四项自审清单

1. **情形表逐行**：B2 环境消息、onboarding 消息、未知/hello 消息、退订后消息分别检查，均通过；B3 root shell 与新增时钟逐字段通过；A5 同时 start 的最终登记 PID 和 live 身份保留严格判据，但真实执行受 ps 限制，未取到通过证据。
2. **失败分支**：恢复 hook 的 loader 持续失败分支通过且不循环；env-page 保留拒绝授权、读盘失败、超时、内核未连接四种原文反馈并通过。三态实例表覆盖拒绝连接、超时、错误回包、PID 复用和 ps 读不到，8 个非监听用例通过。真实进程检查的权限失败原样保留；不适用：本轮未新增产品授权、文件读写或外部服务入口。
3. **全部迁移**：计时器空闲→排队→999 ms 未触发→1000 ms 触发→下一窗口、订阅→退订、offline/connected × 成功/失败及再次离线恢复，相关 45 条通过；live-routes 原有在途叠加、旧完成不清新批、失败后恢复也通过。锁活持有者拒绝接管、死持有者接管和按 token 释放通过；双 start→唯一 serve 的真实迁移留给宿主。
4. **输入域边界**：999/1000 ms、空事件批、未知前缀、两条并发 start、clock 固定 ISO 时间与宿主时区，前四项的非进程部分及 clock 用例均通过；双 start 的边界仅完成测试修正，实跑受限。日历当天/跨天/跨年由 clock 8 条继续守住。本轮没有新增数值/字符串输入域，其余最大值、闰日等不适用。

## 交付

提交时临时使用 `-c core.hooksPath=/dev/null`，避免 post-commit 自行启动禁止的全量门禁。提交失败若为权限拒绝，则提交由 Autome 兜底。所有已关闭里程碑及证据指针保持原样，环境限制不记为协议失败。
