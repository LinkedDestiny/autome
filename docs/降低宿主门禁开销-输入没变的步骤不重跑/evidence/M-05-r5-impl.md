# M-05 · 实现 #5 · 内核导出本轮起点与任务基线

结论：本轮仅推进 M-05，实现轮次 `5/30`。上一轮 M-04 审计通过，无退回缺陷，因此无须搬入审计临时复现检查。

## 实现与范围

- 实现 / Bug 修复会话均在启动编辑进程前记录 `session.started.start_heads`。
- 单仓库 Gate / FixGate 从按事件序号排序的最近对应角色会话取起点，导出 `AUTOME_ROUND_START`；独立计算 `git merge-base <RepoSlot.base> HEAD` 导出 `AUTOME_BASE`。不借用审计会话或更旧轮次的起点；旧事件缺字段时只给基线，merge-base 失败则不提供基线。
- 空布局、单成员或多成员工作区、MergeGate / MainCheck / ReproCheck 不导出二者；底层门禁启动清除继承值后再设置本次值，避免继承宿主别的任务起点。
- 新增 3 个真实内核 + 假 CLI e2e 与 1 个域边界单测。失败场景复制真实 lib.sh / summary.mjs 到临时仓库，使用小步骤表按 git diff 选择真实临时 crate 的测试目标；CLI 提交失败测试后，当轮快检失败、摘要带 changed_case、下一轮修复后通过，未改测试标记始终不存在。预算 120 / 1 秒各走一遍，1 秒分支在第一个真实测试执行 2 秒后明确记录 later 未跑。
- 技术设计 §8.1 同步变量、回退与工作区边界。未改门禁时机、报告格式、并发调度、M-06 或 `.autome/` 配置。

## 验收与用例口径

| 命令 | 用例总数与结果 | 原始输出 |
|---|---|---|
| `cargo test -p automed --test end_to_end gate_quick_` | 9/9 通过，0 跳过；58.03 秒（测试执行阶段） | `.autome/output/m05-e2e-green.txt` |
| `cargo test -p automed --lib gate_quick_host` | 10/10 通过，0 跳过；29.69 秒（测试执行阶段） | `.autome/output/m05-host.txt` |
| `cargo test -p automed --lib gate_round_environment_boundaries` | 1/1 通过，0 跳过；0.38 秒 | `.autome/output/m05-boundaries.txt` |

最终验收与直接相关子集共 **20 个不同用例，20 通过、0 失败、0 跳过**。同范围上一轮代码基线 **16**（e2e 6 + host 10），净增 **4**（3 个 e2e + 1 个边界单测）。设计原来写 e2e 基线 5、预期 8，实际已有 6 个，包括 `gate_quick_feature_removing_quick_while_running_paused_or_stopped`；本轮保留全部 6 个，修正为预期 9。与上一实现轮实际执行的 48 个不同用例相比，本轮执行总数 **-28**：切换为本里程碑的内核子集，不是删除或跳过原有 48 个测试。重复执行与负向实验不重复计数。

开发中的初次 e2e 8 过 / 1 失败（`m05-e2e.txt`），原因是新测试误将“执行 1 失败 + 1 通过”期望为 `steps.ran=1`；现有报告契约是执行总数 2。单跑定位同样失败（`m05-related-debug.txt`），修正为 2 后完整 9 个通过；没有修改产品报告或放松失败、摘要、未跑、未触碰与 passes 断言。

## 判别性负向对照

1. 将生产环境导出的起点临时改为门禁当前 HEAD，原样运行 `cargo test -p automed --test end_to_end gate_quick_feature_related_test_failure_names_case_and_skips_untouched`：退出 101，1 个用例失败（`m05-negative-current-head.txt`）。证明不能用当前 HEAD 空改动集蒙混过关。
2. 临时去掉 Bug Fix 的 start_heads 捕获，原样运行 `cargo test -p automed --test end_to_end gate_quick_bug_fix_gate_exports_round_start`：退出 101，1 个用例失败（`m05-negative-bug-start.txt`）。证明不能只给需求会话记录起点。
3. 两处注入均已还原；最终验收重跑额外设置 `AUTOME_ROUND_START=invalid-inherited-round AUTOME_BASE=invalid-inherited-base`，验证实际启动覆盖或清除宿主继承值。结果记录于下方最终复验。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计 C.1 / M-05 情形 | 执行检查与结果 |
|---|---|
| 需求 Gate 导出会话开始 HEAD | feature_exports：假 CLI 在编辑前独立读取 HEAD，与门禁环境相等，并且不同于门禁 HEAD；通过 |
| 基线为 merge-base | feature_exports / bug_exports：假 CLI 独立运行 merge-base main HEAD 与环境比对；通过 |
| Bug FixGate 与 start_heads | bug_exports：实际 Fix 会话事件与 FixGate 环境一致，且不同于修复提交；通过 |
| 只用最近对应角色 | boundaries：两条 Impl、随后 Audit 与 Fix，Gate 只取最后 Impl；FixGate 只取 Fix；通过 |
| 工作区不导出 | boundaries：单成员具名工作区、多成员、空布局全部为空；通过 |
| 无快检与 each_round 兼容 | 原有 each_round 逐事件对照、host full/legacy、移除 quick 用例全部保留通过 |
| 失败当轮退回、摘要和预算 | related_failure：失败→修复→通过，prompt 与 gate.result.tail 含 changed_case；1 秒预算含未跑 later；未改标记不存在；通过 |
| 子集不能发布整步通过 | related_failure：两轮快检 passes 均为空；失败 / not_run / ran 计数逐项断言；通过 |

C.2 的候选分类表由 M-04 完成，本轮没有改变候选生成器，未把其旧结果冒充本轮实跑。

### 2. 失败分支

- 空输入 / 旧记录：无 session、null/空数组 start_heads、空 head、错误 repo 均运行 boundaries；只给基线且不借用旧起点，通过。
- 基线解析失败：不存在的 base 分支，boundaries 实跑 merge-base 失败，省略基线，通过。
- 相关测试失败：真实 cargo 测试失败且报告失败用例，再经真实调度返回实现轮；通过。
- 预算超限：1 秒预算下已启动测试完整结束，后续目标进入 not_run；通过。
- 拒绝授权、磁盘读取失败、外部服务不可达：不适用；本轮读取既有账本和本地 Git，不新增授权、远程服务或文件报告读取分支。
- 门禁进程超时：不适用本轮新增逻辑；未改既有 poll/timeout 行为，host 队列与模式切换用例照常通过；不声称另做了实际分钟级超时实验。

### 3. 全部迁移

- 需求：Impl→Gate 成功→Audit；Gate 失败→Impl→Gate 成功；多里程碑每轮 quick→MergeGate full→MainCheck full，既有与新增 e2e 均通过。
- Bug：Fix→复现+quick 成功→复核；quick 失败→Fix→成功；MergeGate 仍复现+full，既有 Bug e2e 及新增起点检查通过。
- 快检配置/队列：host 覆盖 full/legacy、quick/full 结果不互用、运行模式切换、删除 quick、排队重选配置与 head；既有 10 个全部通过。
- 导出通道：Gate、FixGate 输出；MergeGate、MainCheck、ReproCheck 不输出；最近事件缺字段回退基线、基线不可解析则省略；boundaries 与 feature_exports 均通过。本轮不新增状态机。

### 4. 输入域边界

- 单仓库空名 slot；零/单/多 slot；单成员具名工作区；空 head、空数组、字段缺失；最近 Impl 与其它角色交错：boundaries 已执行，通过。
- 真实 Git 提交 SHA 与 merge-base：两个 exports 用例已执行，通过；当前 HEAD 错替起点负向对照变红。
- 预算 1 / 120 秒、刚启动后超过预算与剩余候选：related_failure 已执行，通过。预算参数解析本轮未改，0/最大值等由 M-04 负责。
- 前导零、闰日、跨年：不适用；本轮输入不是日期或数字字符串。

## 耗时与未运行范围

上表是本轮验收子集的真实测试耗时，不能冒充文档/Web/Rust 各类产品改动的完整快检墙钟。设计 C.3 要求的各类完整快检耗时本轮**未实测**：用户明确要求实现轮只跑里程碑验收和直接相关子集，不自行运行项目门禁；该测量应由宿主执行对应快检时采集。未运行 `sh scripts/ci-local.sh`、项目快检脚本或宿主 fmt/clippy/Web lint/typecheck 组合，也未运行全量 Rust/Web 测试。不据此声称 C-R2 的产品快检时延已验收。

仅用 rustfmt 格式化 3 个改动的 Rust 文件；最终 `git diff --check` 检查属于非用例验证，不计用例数。

## 最终复验与夹具时序修正

- 额外继承错误环境值的第一次完整复验（`m05-final-e2e.txt`）为 8 过 / 1 失败，失败于 related 场景未到 await_merge。当次断言没有保存终态事件，不能断言由环境变量污染导致。补上 task/event 诊断后，同环境单跑 1/1（`m05-inherited-debug.txt`，57.18 秒），完整重跑 9/9（`m05-final-e2e-2.txt`，21.35 秒）。
- 自审发现小步骤表把 1 秒计时起点放在 git 改动发现之前：若发现过程跨过预算边界，首个失败测试可能合法地未跑，后续假 CLI 步骤不再对应预设角色。这是夹具的具体时序漏洞；不能把机器负载当作正确性保证。改为首个受控候选在 t=0 启动，再使用真实 `ci_quick_admit` 检查后续候选；仍要求实际 cargo 测试失败、摘要带用例名、later 未跑且未改测试不执行，未放松任何断言。生产预算实现没有变化，首候选开始前预算规则属于 M-04 的既有测试。
- 当前主机采样 load average 8.34/9.33/7.77，仅作为负载背景，非失败原因证明。最终相关场景复验见 `m05-related-final.txt`；不重复增加用例总数。

最终结果：`m05-related-final.txt` 1/1 通过、退出 0（38.50 秒），`m05-final-host.txt` 10/10 通过、退出 0（26.19 秒）；最终代码对应 9 个 e2e（完整复验后仅修改 related 夹具并重跑该例）、10 个 host 与 1 个 boundaries 共 20 个绿色用例。`git diff --check` 退出 0。
