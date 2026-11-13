# M-02 · 实现 #2 · 待审

## 范围与实现

对应 K-R3、K-R6、X-4a/b 与设计「K 越界自愈」M-02 四条。上一轮 M-01 审计通过，无退回缺陷，也没有必须逐字搬入的审计失败检查。

- Context.scope_patch_pending 改为 handoff_pending；两处域上下文与启动收件轮均从 handoff::pending 取数据。Handoff 包含 scope_patches、恒空 notes；不提前实现 M-09。
- 将需求简报补丁段与 Bug 修复 prompt 段收进 handoff::sections。已关闭表追加禁止修改里程碑表、已完成证据指针以及本轮 evidence 文件路径；空补丁不输出。
- integration.range 根据 session.started.scope_patches 标记 scope_patch；集成审计摘要与简报两处映射均显示「越界补丁交接」。
- README 与技术设计 §7.4 搬入 refs/keep/t69-scope-self-heal 的文档差异；§7.4 后补交接轮、三条转换边、预算、熔断、集成复验和未来备注接入边界。未改协议、账本结构、网页或其它里程碑。

## 验收命令、结果与基线

所有命令在当前 worktree 执行；测试使用项目原有临时账本与临时仓库夹具，未启动真实账本、未跑全量门禁或宿主快检。

| 命令 | 最终结果 | 原始输出 |
|---|---|---|
| cargo test -p automed handoff_ | 4/4（库 3、e2e 1），退出 0 | .autome/output/m02-accept-green.txt |
| cargo test -p autome-domain scope_patch | 13/13，退出 0 | .autome/output/m02-domain.txt |
| grep -q 'session.scope_restored' README.md && grep -q '上一次已自动还原' docs/development/technical-design.md && grep -q '里程碑关闭之后的交接轮' docs/development/technical-design.md | 三项通过，退出 0 | 命令直接返回 |
| cargo test -p automed --lib -- scope::tests::restore_ session_scope_ scope_patch_ task_get_scope_restore | 19/19，退出 0 | .autome/output/m02-regression-lib.txt |
| cargo test -p automed --test end_to_end -- scope_restore_ scope_breaker_ review_scope_ integration_e2e_ | 20/20，退出 0 | .autome/output/m02-regression-e2e.txt |
| cargo test -p automed --lib handoff_ | 负向对照还原后 3/3，退出 0 | .autome/output/m02-negative-restored.txt |

**本轮去重用例总数 56，失败 0、跳过 0。** 验收契约本身 17 例（4 + 13），相关子集 39 例（19 + 20）。上一轮实跑基线 52，本轮差 +4：新增 handoff 4 例，扩大相关范围加入既有 integration_e2e 4 例，本轮未涉及 CLI 未重复跑其 4 例，合计 52 + 4 + 4 - 4 = 56。同范围代码用例净增 4；重复运行和故障注入不重复计数。grep 三项不是测试用例。域原有 13 例名字与断言保留。

运行过 cargo fmt --all（格式化，非宿主检查）及 git diff --check；均通过。开发阶段的编译错误已修正，不作为通过证据。未运行 cargo clippy、Web lint/typecheck 或 scripts/ci-local.sh，由宿主按协议执行。

## 红绿与负向对照

1. 新增 e2e 初次运行时，库 3 例通过，e2e 在「审计简报包含越界补丁交接」断言失败（.autome/output/m02-accept.txt）。原因是 integration 摘要和 brief 独立渲染；只补前者不足。补齐 brief 映射后，同命令 cargo test -p automed handoff_ 四例全绿（m02-accept-green.txt）。断言留在项目测试体系。
2. 将 integration_trigger 的 scope_patch 返回值临时改为 none，同时删掉 sections 的补丁轮限制句；cargo test -p automed --lib handoff_ 退出 101：pending 通过，trigger 与 closed-table 两例失败（.autome/output/m02-negative.txt）。还原两份文件后原命令 3/3 通过（m02-negative-restored.txt）。故障注入已全部撤销。
3. e2e 验证真实调度器/Git 的完整链路：审计越界新增文件 → 内核还原 → 实现轮先确认文件不存在再采纳产品改动 → integration.range.trigger.kind == scope_patch → 集成审计 prompt/brief → await_merge，且已处理事件恰好一条。终点是合并卡点，不声称已合并主分支。

## 标待审前自审清单

### 1. 情形表逐行

本里程碑没有另列情形表，按设计 M-02 四行和共用机制逐项检查：

| 设计点 | 检查与结果 |
|---|---|
| handoff 上下文与按账本顺序取补丁 | pending 单测包含空账本、乱序命名、已处理、失败和其它任务隔离；通过。两处 Context 均改名，旧 scope::pending 只作为内部数据来源。 |
| 触发标签与集成复验 | trigger 单测、真实 e2e 的 range/prompt/brief 断言；通过。 |
| 全关闭表的补丁轮说明 | sections 单测验证完整限制句与 M-16-r80-impl.md；e2e 检查实际 impl-2 简报的 M-01-r2-impl.md；通过。 |
| README / 技术设计 | 三条契约 grep 通过；人工对照三条边、预算、同角色连续熔断、手动复盘、会话 Ok 才结算，均已记录。 |
| 只改文档仍普通审计 | integration_e2e_doc_only_repair_keeps_todays_audit 通过。 |
| 原补丁路径、选择是否采纳、Bug 文案保持 | 19 库例与原 scope_restore e2e 子集通过；新 sections 用例分别检查需求与 Bug 文案。 |

### 2. 失败分支

- K-R5 还原受阻：restore_refuses_a_dirty_worktree_and_leaves_it_untouched、restore_patch_dir_blocked_changes_nothing、restore_workspace_failure_in_one_repo_compensates_the_other 全部通过；不改既有失败传播。
- K-R4 连续越界熔断：scope_breaker 与 session_scope_manual_retro_second_crossing_fails 通过；同角色干净会话清零用例通过。
- 崩溃不消费：scope_patch_crashed_impl_is_presented_again_on_next_launch 通过；Ok 结算仅限已呈现补丁用例通过。
- 空输入：pending 空账本、sections 空数组不输出、trigger 缺字段或空数组为 none，均通过。
- 拒绝授权、外部服务不可达、超时：不适用；本里程碑不新增授权、网络或超时分支。文件读取失败沿用已有 status 读取 Option 行为，没有新增持久化失败分支。

### 3. 全部迁移

- Audit × 有补丁 × 全完成 → Implement；无补丁 → Retro/Rebase；关闭表预算耗尽仍补实现；开放表继续服从预算：对应域 scope_patch 用例全部通过。
- Retro × 有补丁 → Implement；无补丁 → Rebase：域用例及真实复盘交接 e2e 通过。
- Bug FixReview × 通过 × 有补丁 → Fix；无补丁 → Rebase；有补丁而预算耗尽 → Failed：域用例及 Bug 复核交接 e2e 通过。
- 已呈现补丁 × Ok → handled；失败/崩溃 → 保留并重投；还原中断 → 接续：19 库例覆盖并通过。
- 交接实现 × 产品提交 → 集成审计；文档提交 → 普通审计：新 handoff e2e 与既有 integration_e2e_doc_only 用例通过。

### 4. 输入域边界

- 空列表、单补丁、多个补丁账本顺序、失败/已处理过滤、其它任务隔离：新增两条 handoff 单测通过。
- 缺省/空 scope_patches → none；非空 → scope_patch：trigger 单测通过。
- 预算 0、2、u32::MAX 与已耗尽边界沿用域测试通过；文件名的空格、非 ASCII、glob 字符、二进制与模式位由既有 restore 用例通过。
- 日期、前导零、闰日、跨年：不适用；本轮无此类输入或解析。

## 交付

四项自审已执行，M-02 可标待审。只推进 M-02；implementation-round 按调度值写 2/80，等待独立审计。提交在本轮结束前尝试；若权限拒绝则提交由 Autome 兜底。
