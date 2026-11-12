# M-04 · 实现 #4

本轮仅推进 M-04（设计 §6.1、§6.2、§6.9）。上一轮审计 #3 通过 M-03，没有退回缺陷；因此没有需要逐字搬入测试体系的审计检查。

## 实现

- 将原有结果渲染函数原样移至 `brief::gate_line_of_result`，输出文字不变。
- 新增 `gate_evidence`：配置完整门禁且使用 before_merge 时，仅最后一条结果的 SHA 与当前 HEAD 相等才引用；无结果、旧 SHA、读不到 HEAD 时写固定说明。each_round 和未配门禁保留旧逻辑。
- 调度器在写审计简报之前，用既有 `gate_head` 计算身份并传入证据；其它角色仍传 None；`brief::build` 不改。
- 新增 3 个 brief 单测、1 个整份简报金样用例、1 个真实 Git/worktree 的 e2e。现有用例及 prompt 金样未改。

## 验收命令与用例数量

验收命令（顺序执行，均退出 0）：

1. `cargo test -p automed --lib brief`：33 个通过，上一轮对应基线 30，增加 3 个判定矩阵单测。
2. `cargo test -p automed --test prompt_rendering`：17 个通过，基线 16，增加 1 个审计简报金样。
3. `cargo test -p automed --test end_to_end brief_before_merge`：1 个通过，基线 0，增加 1 个旧结果隔离 e2e。
4. `cargo test -p automed --test end_to_end a_failing_gate_sends_the_work_back_and_a_passing_one_lets_it_merge`：1 个通过，基线 1，未修改。
5. `cargo fmt --all -- --check` 与 `git diff --check`：通过。

本里程碑验收共 52 个用例，相对相同范围基线 47 个增加 5 个，失败 0、忽略 0。上一轮 M-03 验收范围为 16 + 91 + 12 = 119 个，与本轮过滤范围不同，不能将 119 到 52 当作用例减少。测试清单规模可由输出交叉核对：automed lib 663（33 + 630 filtered），上一轮 660，增加 3；end_to_end 119（1 + 118 filtered），上一轮 118，增加 1；prompt_rendering 17，上一轮 16，增加 1。

原始输出：`.autome/output/r4-{brief,prompts,e2e,compat}.txt`；故障还原后复验输出：`.autome/output/r4-{brief,prompts,e2e,compat}-final.txt`。格式检查：`.autome/output/r4-fmt-check.txt`。全量 workspace / clippy / ci-local 不在本轮自跑范围，由宿主门禁负责，未在本轮宣称通过。

## 标待审之前的自审清单

### 1. 情形表逐行

对照 §6.2，均由新增 brief 单测实际执行：

| 情形 | 检查与结果 |
|---|---|
| 配置门禁 + before_merge + 无结果 | 返回完整固定说明，含句号和换行 |
| 配置门禁 + before_merge + 旧 SHA | 返回固定说明，不引用旧结果 |
| 配置门禁 + before_merge + 同 SHA | 成功和失败分别与原渲染函数相等 |
| 配置门禁 + before_merge + HEAD 不可读 | 传 None，返回固定说明 |
| each_round | 无结果/有结果 × HEAD 缺失/同 SHA/不同 SHA，逐字节等于旧逻辑 |
| 无门禁 + before_merge | 同上述矩阵，逐字节等于旧逻辑 |

固定输入的审计简报整份与 `golden/briefs/audit-before-merge.md` 比对。e2e 检查第一轮与修复后的第二轮审计均有宿主门禁标题和固定说明，且不含失败提交 SHA 前 10 位、不含“：未通过”。

### 2. 失败分支

- 设计点名的读取 HEAD 失败：纯函数输入 None，覆盖有旧结果、无结果、结果缺少 SHA 三种情况，均显示固定说明；调度器使用 `gate_head(...).ok()` 接入该分支。
- MergeGate 失败：真实门禁脚本因缺少 gate-ok 返回失败，e2e 检查失败事件、修复 prompt 的门禁尾部和命令，再生成新提交，第二次审计不引用旧结果。
- 拒绝授权、超时、外部服务不可达：不适用，M-04 不新增这些执行分支；门禁执行器不改。
- 空输入：无 gate.result 与无 HEAD 已执行，不以两个缺失值相等认定为本轮结果。

### 3. 全部迁移

M-04 不新增状态机；本轮 e2e 实跑并断言 MergeGate 失败之后节点序列：
`implement → audit → retro → rebase → merge_gate → await_merge`，随后合并到 done。
首次审计无结果 → MergeGate 失败产生结果 → 修复提交改变 HEAD → 再次审计拒绝旧结果，全部经过真实调度。
同 HEAD 成功/失败结果仍可显示，两条分支由单测执行。each_round 原有完整门禁失败退回、成功后合并由未修改的兼容 e2e 执行。

### 4. 输入域边界

- Option 边界：last/head 各自缺失、同时缺失、结果缺少 SHA，均执行。
- SHA 显示边界：1 字符、恰好 10 字符、超过 10 字符，对成功/失败分别执行；保持既有截断规则。
- 配置边界：门禁存在/不存在 × 相应模式已覆盖；非审计角色仍不注入证据。
- 前导零 SHA 已包含于 10/16 字符样本。闰日、跨年、数值最大值不适用，本功能没有日期或数值域输入。

## 负向对照与还原

三组设计指定故障注入均保存原文件，并用 ensure 还原；不更新金样：

1. 将 SHA 相等条件临时替换成 `!head.is_empty()`，使旧结果被沿用。
   - `cargo test -p automed --lib brief::tests::before_merge_missing`：退出 101，1 个失败。
   - `cargo test -p automed --test end_to_end brief_before_merge`：退出 101，1 个失败，第二轮简报出现旧失败证据。
   - 输出：`r4-negative-stale-unit.txt`、`r4-negative-stale-e2e.txt`。
2. 固定说明的“运行”临时改为“执行”。
   - `cargo test -p automed --test prompt_rendering before_merge_audit_brief_matches_its_golden`：退出 101，1 个失败，逐字节比对捕获文案变化。
   - 输出：`r4-negative-golden.txt`。
3. 临时去掉 each_round 兼容条件。
   - `cargo test -p automed --lib brief::tests::before_merge_each_round`：退出 101，1 个失败，无结果由 None 错变成固定说明。
   - 输出：`r4-negative-each-round.txt`。

上述输出均位于 `.autome/output/`。还原后重跑完整 M-04 验收命令，恢复 52 个通过。无遗留故障注入，无需修改设计方案。

结论：M-04 待独立审计；无阻塞。
