# M-03 实现轮 #4 证据

本轮只推进 M-03。上一轮审计 #2 对 M-02 的结论为通过，无退回缺陷，因此没有需要逐字迁入并红绿复现的审计检查。

## 实现与范围

- 钩子启动前扫描其它任务的 `after_merge-*.started` 与 `setup-main` 作业，使用 `gate::running` 判断占用，覆盖 wrapper 尚未写 pid 的启动窗口；作业结束即放行，不要求对方已收结果。
- 停止和取消复用原有进程组终止路径，并删除 `after_merge-` 作业文件；保留 `after_merge.prepared`，恢复时不重复主检出准备。
- 沿用现有准备完成标记、跳过判定及 Bug/Cleanup 调度，不改变 setup 的互斥语义。
- 新增约定 E4–E8、L3、L5 共 7 个测试函数；E8 内覆盖停止、暂停、取消三个分支，L3 内额外检查无合并提交、MainCheck skipped 及跳过结论重入。

## 命令与结果

全部退出码为 0，测试结果 0 失败、0 忽略；原始输出保存在 `.autome/output/`。

| 命令 | 结果与日志 | 上一轮基线差 |
|---|---|---|
| `cargo test -p automed --test end_to_end` | 111 通过；`m03-e2e.txt` | 106 → 111，+5（E4–E8） |
| `cargo test -p automed --lib` | 657 通过；`m03-lib.txt` | 655 → 657，+2（L3、L5） |
| `cargo test -p automed --test end_to_end merge_after` | 8 通过；`m03-e2e-focus.txt` | 3 → 8，+5 |
| `cargo test -p automed --lib merge_after` | 10 通过；`m03-lib-focus.txt` | 8 → 10，+2 |
| `cargo fmt --all -- --check` | 通过；`m03-fmt-check.txt` | 无格式差异 |
| `cargo clippy --all-targets -- -D warnings` | 通过；`m03-clippy.txt` | 无告警 |
| `git diff --check` | 通过 | 无空白错误 |
| `git diff --exit-code HEAD -- .autome/` | 通过；`m03-autome-diff.txt` 为空 | 本轮无受跟踪配置改动；仅按协议写 output 日志 |

本轮验收两个完整目标合计 **768 个用例**，相对上一轮审计基线 **761 个增加 7 个**，全部来自上述新增测试函数，没有删除或改名旧用例，没有引入其它来源的用例。专项过滤为这 768 个的子集，不重复计数。E8 的三条用户操作分支与 L3 的附加跳过场景均在已有新增函数内运行，不虚增测试函数总数。

本轮没有运行 workspace 全量与 Desktop/Web 门禁；这些属于 M-04。没有运行真实用户驱动安装与人工界面验收。

## 标待审之前的自审清单

1. **情形表逐行**（全部已执行并通过）：
   - §4 工作区 → E6，一条跳过事件、无作业产物；无合并提交 → L3 附加场景，一条指定原因事件，重复 Cleanup 不重写。
   - §4 MainCheck Failed → E4，只有 main_gate_failed 警报、不启动钩子；Skipped → L3 附加场景，指定原因、无启动；NotRun → E7 Bug 任务照常跑；Passed → E1 主检出执行且序号正确。
   - §6 别人钩子 / setup-main：L3 对两者分别检查只有 started 无 pid、活 pid、结束标记三阶段，前两者等待，结束后启动。
   - §7 主检出 setup 成功 / 失败：E5 两段，准备结果先于钩子、主检出准备只运行一次；失败仍运行钩子且 setup_failed 恰一条。L5 在其它钩子占用期间连续重入四次，prepared 存在，准备次数和警报保持一，再释放并完成。
   - §8 生命周期各行见第 3 项；未配 / 空白命令 → E3 两段，零事件、零钩子文件。
2. **失败分支**（已执行并通过）：MainCheck 非零退出 E4；主检出准备 exit 4 E5/L5；钩子 exit 6 E2（仅报警、Done）；钩子超时 L1（124、子进程终止）；wrapper 消失 L2（清除再跑、无失败结论）；失败结论写入失败 S1（事务全部回滚）。空命令 E3 和配置解析用例覆盖。授权拒绝、外部服务不可达不适用：没有新增授权/服务接口；读盘失败不适用独立业务分支：设计未定义新的读盘失败降级语义，沿用 Result 传播，本轮未声称注入取证。
3. **全部迁移**（已执行并通过）：
   - Cleanup 未准备 → setup Pending → Ready/Failed → prepared → 钩子 Pending → Done：E5/L5；互斥等待 → 解锁启动：L3/L5。
   - 未启动 → Skip → 清理 Done：E4/E6/L3；跳过后重入 → 不重复结论：L3。
   - 启动 → Running → 成功/非零/超时 → 收集 → Done：E1/E2/L1/L2；作业 Vanished → clear → 下一 tick 重启：L2。
   - Active Cleanup → Paused → Resume → 收原作业：E8 的 pause 子场景，pid 活着、开始一次；Active Cleanup → Stopped → Resume → 新作业：E8 stop，旧 pid 消失、作业文件删除、开始两次且结果一次；Active Cleanup → Cancelled：E8 cancel，进程死亡、文件删除、无结果无警报。
   - core 重入接作业、结论已写但尚未 Done 时重入：L2；作业已启动但事件缺失时补开始事件：L4；停留期间删配置 → 直接清理、作业后台完成且不收结果：L2。重启沿用约定的直接重入检查，未实际重启用户驱动。
4. **输入域边界**（已执行并通过）：M-03 输入为项目形态、MainVerdict、作业生命周期和任务操作；单仓/工作区、commit 有/无、NotRun/Passed/Failed/Skipped、无 pid/活 pid/结束标记、暂停/停止/取消均见上述检查；单一占用任务和释放后的零占用由 L3 验证。空命令 E3、配置缺省/非法值由现有 lib 配置测试覆盖。日期、闰日、跨年、前导零与数值最大值不适用：M-03 未引入日期或数值解析域，超时解析属于已关闭 M-01。

## 结论

M-03 验收与四项自审通过，提交独立审计，状态仅标为待审。M-04 保持开放。
