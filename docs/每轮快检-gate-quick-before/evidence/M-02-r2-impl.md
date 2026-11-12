# M-02 实现 #2

结论：本轮只推进 M-02，宿主需求快检与沿用隔离已实现，验收通过后提交独立审计。

## 改动与范围

- 需求 Gate 在 before_merge + quick 时运行快检；each_round 保留完整门禁。沿用现有准备步骤、队列、后台进程组、日志与失败回退。
- 新增 Launch 的 .kind 标记：quick 固定启动命令与超时；full/repro 保持旧命令记账逻辑。start/clear 保留标记，discard 清除；启动事件、结果事件与 AUTOME_GATE_KIND 来自实际启动种类。
- MergeGate 与 MainCheck 均拒绝沿用 quick；仅涉及 quick 的种类切换隔断同因连续失败。full/repro 和无 kind 旧事件仍按原规则累计。
- 巡检使用 quick 标记里的超时。未实现 M-03 的 Bug 快检命令选择，未改 M-04/M-05 的文字、协议、界面与文档。
- 上轮审计 M-01 通过，没有退回缺陷或需逐字搬运的判别检查；现有测试及断言未修改。

## 命令与结果

原始输出保存在 .autome/output/。下列验收总数 27（19 + 8），均通过，0 失败、0 忽略。
与上一实现轮验收总数 29 相比为 -2：本轮换成 M-02 的宿主/e2e 范围，不是删除两个用例。
以本轮同一筛选范围的既有基线 13（9 库 + 4 e2e）比较，新增 14（10 库 + 4 e2e）。
比设计原预计 25 多 2：增加排队/新 head 场景库测试，以及在跑/暂停/停止后移除 quick 的 e2e。

| 命令 | 结果 | 日志 |
|---|---|---|
| cargo test -p automed --lib -- gate_quick_host before_merge_fix_gate before_merge_unconfigured before_merge_repro_marker main_check_reuse | 退出 0，19 用例 | m02-lib-final.txt |
| cargo test -p automed --test end_to_end -- gate_quick_feature gate_quick_each_round feature_before_merge | 退出 0，8 用例 | m02-e2e-final.txt |

## 标待审之前的自审清单

### 1. 情形表逐行

对照设计 §3、§7 的需求部分：

| 情形 | 执行检查与结果 |
|---|---|
| each_round（有/无 quick） | full_and_legacy_jobs_remain_full + each_round e2e；快检调用 0，全部门禁事件种类/node/kind 序列相同 |
| before_merge + quick | runs_quick_and_records_failures；started/result 为 quick，脚本读到 quick，标记超时 1 分钟 |
| before_merge 无 quick | 4 个原 feature_before_merge e2e 不改通过 |
| MergeGate / MainCheck | two_milestones e2e；快检 2 次、MergeGate full 1 次，合并后 MainCheck full 再 1 次 |
| quick 在跑改为 each_round | mode_switch_collects_by_launch_kind；仍按 quick 的原命令收结果 |
| quick 在跑改命令 | 同上；结果仍为 original 命令，不变成新配置 exit 99 |
| quick 在跑改超时/模式 | queue_reclaims_with_launch_timeout；当前 full 超时 30，启动 quick 超时 1，61 秒记录触发终止 |
| full 在跑改 before_merge + quick | full_and_legacy_jobs_remain_full；仍是 full 和完整命令，包含无标记旧作业 |
| quick 在跑/暂停/停止时拿掉 quick | removing_quick e2e 三条路径；进程组已消失、作业文件清除、gate.skipped 后进入审计，再跑完整 MergeGate |
| 排队中切换/移除 quick | queued_jobs_select_current_config_and_head；真正排队且未启动，释放名额后按当前配置跑 full、新 quick 或跳过 |
| 作业期间 head 变了 | 同一库测试；旧槽及 kind 清除，无旧结果，启动绑定新 head 的新命令 |
| 审计等其它节点加 quick | 不适用于本轮修改点：实现轮完成路由由已通过的 M-01 管理，本轮不改变状态机 |
| Bug 专属情形 | 不适用：M-03 实现；本轮保留的 5 个 before_merge Bug/标记回归用例通过 |

### 2. 失败分支

- 检查失败：runs_quick_and_records_failures 覆盖退出 9、126、127，结果绑定原命令与 quick；失败 e2e 确认退回实现、prompt 带原命令和标记尾部，修复后继续。
- 超时：真实后台 sleep 进程，调整 started 时间后由队列巡检终止，退出 124，日志保留“超过 1 分钟”内核标记，收结果仍为 quick。
- 标记写盘失败：launch_marker_lifecycle 用同名目录阻挡 .kind 文件写入，set_launch 返回错误；产品调用映射“无法记录门禁种类”后停止启动。
- 缺失/不可读取标记：新 Job 无标记返回 None；full_and_legacy_jobs_remain_full 删除旧标记后实际收结果为 full，保留旧行为。
- 拒绝授权、空输入、外部服务不可达：不适用，本轮没有授权交互或外部服务客户端，命令输入校验由 M-01 管理；shell 非零状态统一走已测失败路径。

### 3. 全部迁移

- Gate 成功→后续审计→下一实现/合并，Gate 失败→实现修复→Gate 再成功：分别由两个需求 e2e 跑通；each_round 对照也合并至 done。
- §7 需求在途、排队、暂停、停止、head 变化路径均见上表；无新增状态机节点。
- MergeGate 同 head：quick 拒绝沿用并真实启动 full；full 对照仍沿用。
- MainCheck：直接 result 与 reused 间接指针各测 quick 拒绝/full 接受，共 4 格。
- 熔断：前后 kind 各取无值/quick/full/repro，共 16 格；先两次同因失败、第三次切换，只有涉及 quick 的异类重置到 1，其余第三次熔断。
- 标记生命周期：无标记→set→start 保留→discard 删除；再次 set→clear 保留→discard_stale 删除。

### 4. 输入域边界

- kind 的 quick/full/repro/缺省由启动、旧作业及 16 格熔断表覆盖。
- Launch 命令含字面量 {slug}，最大 u32 超时可序列化并跨 start/clear 保留；实际执行用最小有效分钟值 1，对照 full 30。
- 成功退出 0、普通失败 9、126/127 与内核 124 已检查。
- 空 quick、非字符串、0/负数超时属于已完成 M-01 的解析范围，本轮未改；日期、前导零等不适用，本轮无此输入域。

## 负向对照与修复说明

本轮主动注入后恢复，未修改现有断言，也未保留缺陷：
- 删除 MergeGate 的 kind 排除条件：单跑 gate_quick_host_merge_gate_never_reuses_a_quick_result，退出 101，1/1 失败；日志 m02-negative-reuse.txt。
- 用当前选择替代 job.launch：单跑 gate_quick_host_mode_switch_collects_by_launch_kind，退出 101，1/1 失败；日志 m02-negative-launch.txt。
- 删除 Gate 跳过条件中的 quick 检查：单跑两个 gate_quick_feature 主路径 e2e，退出 101，2/2 失败；日志 m02-negative-skip.txt。
- 完全恢复源码（cmp 与注入前副本相同）后，两个库判别用例和两个 e2e 分别退出 0、2/2 通过：m02-restored-lib.txt、m02-restored-e2e.txt。复跑不重复计入 27 个唯一验收用例。

## 最终静态检查与边界

- cargo fmt --all -- --check：退出 0（m02-fmt-check.txt）。
- cargo clippy --all-targets -- -D warnings：退出 0（m02-clippy-final.txt）。
- git diff --check：退出 0。
- 未运行 sh scripts/ci-local.sh 或其它项目全量门禁；未改 apps/web，不运行其 lint/typecheck；四段命令计时归 M-05。
- 只在当前 worktree 修改上述代码、测试和本轮文档；.autome 下仅按用户要求写 output 日志/临时验证文件，没有修改配置、规则或脚本；未推送、未启动下一会话。
