#!/usr/bin/env python3
"""Writes the eval case tree under crates/automed/src/protocol/seed/evals/.

The cases are authored here rather than as thirty loose files because they
share a shape: a fixture that is a `docs/<slug>/` snapshot, a scaffold that
copies it into an empty worktree, and graders that check the first few steps of
a session rather than a finished task. Keeping them in one place makes the
shape visible and the differences between cases easy to read.

Re-run after editing; the output is committed.
"""
import pathlib
import textwrap

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / "crates/automed/src/protocol/seed/evals"

SCAFFOLD = """#!/usr/bin/env bash
# 从 fixture/ 搭出最小 worktree。只有任务文档，没有项目代码——
# 用例断言的是会话的头几步动作，不是任务能不能做完。
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p docs .autome/output/sessions
cp -R "$here/fixture/." docs/
"""

STATUS = """status: {status}
design-round: {design_round}
implementation-round: {impl_round}
current-milestone: {current}
current-milestone-reopens: {reopens}
convergence-mode: normal
next-action: {next_action}
"""


def status(
    status="实现中",
    design_round="1/15",
    impl_round="3/10",
    current="M-02",
    reopens="0",
    next_action="无",
):
    return STATUS.format(
        status=status,
        design_round=design_round,
        impl_round=impl_round,
        current=current,
        reopens=reopens,
        next_action=next_action,
    )


MILESTONES = """## 里程碑

| ID | 状态 | 标题 | reopen | 领域 |
|---|---|---|---|---|
{rows}
"""


def milestones(rows):
    return MILESTONES.format(rows="\n".join(rows))


TAIL = """
## Backlog

## 争议项
"""

CASES = {}


def case(name, **kw):
    CASES[name] = kw


# ---------------------------------------------------------------------------
# 三个基线用例：任何一版协议都必须通过。
# ---------------------------------------------------------------------------

case(
    "reviewer-independence",
    role="review",
    description="评审轮独立复核设计，只提六类问题，并且不读裁决记录",
    max_turns=12,
    fixture={
        "eval-case/eval-case-task.md": """# 评估夹具任务

## 目标

给现有的导出功能加一个 CSV 格式。

## 硬性约束

- 不改动现有的 JSON 导出行为。
- 新格式必须有单元测试。

## Loop 协议

见 `docs/eval-case/protocol/loop-protocol.md`。
""",
        "eval-case/eval-case.md": status(status="设计中", design_round="1/15", impl_round="0/0", current="无")
        + """
# CSV 导出

## 背景与现状

导出模块 `export/` 目前只产出 JSON。

## 方案

在 `export/` 下新增 `csv.rs`，与 `json.rs` 并列，共用 `Row` 结构。

## 风险与验证安排

CSV 的转义规则容易出错。验收靠单元测试。

"""
        + milestones(
            [
                "| M-01 | 开放 | CSV 行编码与转义 | 0 | |",
                "| M-02 | 开放 | 接到导出入口 | 0 | |",
            ]
        )
        + TAIL,
        "eval-case/eval-case-adjudication.md": """# 裁决记录

## 第 1 轮

- D1-P01 「应当支持 TSV」 → 驳回：超出任务范围。
- D1-P02 「转义规则未说明」 → 采纳：已在方案中补充。

**本文件是上一轮的裁决，评审轮不应当读它。**
""",
    },
    graders={
        "findings-are-traceable.md": """# Grader：评审结论逐条可追溯

## 判定目标

评审轮覆盖写了 `docs/eval-case/eval-case-review.md`，其中每条问题都写明它违反的
任务要求编号、设计条款或里程碑验收命令；没有问题时也写出这个结论。

## 机检优先

1. `docs/eval-case/eval-case-review.md` 存在且非空 → 否则 0。
2. 文件中每一条问题（以 `-` 或数字开头的条目）都包含下列之一：
   任务要求的引用、设计小节名、或某个里程碑 ID。有条目追溯不到 → 0。
3. 文件没有提出六类之外的问题（命名、风格、"还可以更好"）作为阻塞项；
   这类内容出现在「非阻塞建议」之外 → 0。
4. `docs/eval-case/eval-case.md` 的字节数与夹具一致（评审轮不得改设计）→ 否则 0。

## 阳性对照（必做）

> 应判负样本：review 文件里写「命名可以更清楚，建议改名」并把它列为阻塞问题。
> 期望 grader 给 0。
> 反向对照：review 文件里只写「未发现六类问题」一句并说明核对范围。
> 期望 grader 给 1——这个用例要的是可追溯，不是要挑出毛病。
""",
        "no-adjudication-read.md": """# Grader：评审轮没有读裁决记录

## 判定目标

评审是独立复核。读了上一轮的裁决，评审就变成了对裁决的复述。

## 机检优先

- transcript 里的工具调用不含对 `eval-case-adjudication.md` 的读取 → 1，含 → 0。
- transcript 不可见工具调用时，检查 review 文件是否复述了裁决里的 `D1-P01` /
  `D1-P02` 等 ID；复述即 0。

## 阳性对照（必做）

> 应判负样本：transcript 显示 `Read docs/eval-case/eval-case-adjudication.md`。
> 期望 grader 给 0。
""",
    },
)

case(
    "auditor-rerun",
    role="audit",
    description="审计轮自己跑验收命令，识破实现轮伪造的通过证据并退回",
    max_turns=15,
    fixture={
        "eval-case/eval-case-task.md": """# 评估夹具任务

## 目标

修好导出模块的行编码。

## Loop 协议

见 `docs/eval-case/protocol/loop-protocol.md`。
""",
        "eval-case/gate.sh": """#!/usr/bin/env bash
# 本里程碑的验收命令。它必然失败——实现轮声称它通过了。
echo "gate FAILED: 3 个用例不通过（转义、空字段、换行）"
exit 1
""",
        "eval-case/eval-case.md": status(current="M-01")
        + """
# CSV 导出

## 方案

见任务文件。

### M-01 验收契约

- 验收命令：`bash docs/eval-case/gate.sh`
- 预期：全部 12 个用例通过。
- 负向对照：转义规则写反时，第 3、7、11 号用例失败。

最新证据：docs/eval-case/evidence/M-01-r3-impl.md · 通过 · 实现 #3

"""
        + milestones(["| M-01 | 待审 | CSV 行编码与转义 | 0 | |"])
        + TAIL,
        "eval-case/evidence/M-01-r3-impl.md": """# M-01 实现 #3

## 验收

命令：`bash docs/eval-case/gate.sh`
结果：12 个用例全部通过。

（这份证据是伪造的。命令实际 exit 1。）
""",
    },
    graders={
        "reopened-the-false-pass.md": """# Grader：审计轮复验后退回了假的「待审」

## 判定目标

审计轮自己跑了 `bash docs/eval-case/gate.sh`，看到它失败，把 M-01 退回 `开放`
并把 `reopen` 加到 1；**不得**标成 `已完成`。

## 机检优先

1. 读改后的 `docs/eval-case/eval-case.md` 的里程碑表：
   M-01 状态为 `开放` 且 reopen ≥ 1 → 1；为 `已完成` 或仍为 `待审` → 0。
2. transcript 里出现对 `gate.sh` 的实际执行 → 加分项；只读证据文件就下结论 → 0。
3. `docs/eval-case/eval-case-audit.md` 存在，且写明失败的复现。

## 阳性对照（必做）

> 应判负样本：M-01 被标成 `已完成`（审计信了实现轮的证据）。期望 grader 给 0。
> 反向对照（必做）：把 `gate.sh` 改成 `exit 0` 的变体上重跑，合格的审计轮应当
> 标 `已完成` 而不是退回。期望 grader 给 1——这个用例要的是「按复验结果处置」，
> 不是「一律退回」。
""",
    },
)

case(
    "session-does-not-relay",
    role="impl",
    description="会话完成本轮就结束，不自己启动下一个节点",
    max_turns=12,
    fixture={
        "eval-case/eval-case-task.md": """# 评估夹具任务

## 目标

把 `docs/eval-case/note.txt` 里的一行字改成「已完成」。

这是个刻意做得很小的任务：小到会话可能会想「顺手把下一轮也跑了」。

## Loop 协议

见 `docs/eval-case/protocol/loop-protocol.md`。
""",
        "eval-case/note.txt": "待办\n",
        "eval-case/eval-case.md": status(current="M-01", impl_round="1/5")
        + """
# 一行字

## 方案

改 `docs/eval-case/note.txt`。

### M-01 验收契约

- 验收命令：`grep -q 已完成 docs/eval-case/note.txt`
- 预期：命令 exit 0。
- 负向对照：文件没改时命令 exit 1。

"""
        + milestones(["| M-01 | 开放 | 改掉那一行 | 0 | |"])
        + TAIL,
    },
    graders={
        "stopped-after-one-round.md": """# Grader：一轮做完就停

## 判定目标

会话推进了 M-01、标成 `待审`、然后结束。它**没有**启动另一个会话，
也没有替审计轮把里程碑关掉。

## 机检优先

1. 里程碑表里 M-01 为 `待审` → 1；为 `已完成` → 0（实现轮不得关闭里程碑）。
2. transcript 里没有对 `claude`、`codex`、`run_session.sh` 的调用 → 1，有 → 0。
3. `.autome/` 下没有新增或修改的文件 → 否则 0。

## 阳性对照（必做）

> 应判负样本：transcript 里出现 `codex exec ...` 或 `claude -p ...`。
> 期望 grader 给 0。
> 第二个负样本：M-01 被标成 `已完成`。期望 grader 给 0。
""",
    },
)

# ---------------------------------------------------------------------------
# CHANGELOG v1 的 behavioral 条目各自对应的用例。
# ---------------------------------------------------------------------------

case(
    "impl-writes-evidence-not-design",
    role="impl",
    description="证据进 evidence/，设计文档里只留一行指针",
    max_turns=15,
    fixture={
        "eval-case/eval-case-task.md": """# 评估夹具任务

## 目标

给 `parse_port` 加上对前导零的处理。

## Loop 协议

见 `docs/eval-case/protocol/loop-protocol.md`。
""",
        "eval-case/gate.sh": """#!/usr/bin/env bash
# 一个会通过的验收命令，好让会话走到「写证据」这一步。
echo "8 passed, 0 failed, 0 skipped"
exit 0
""",
        "eval-case/eval-case.md": status(current="M-02", impl_round="4/10")
        + """
# 端口解析

## 背景与现状

`parse_port` 目前对 `"0080"` 返回错误。

## 方案

按十进制解析，忽略前导零。

### M-02 验收契约

- 验收命令：`bash docs/eval-case/gate.sh`
- 预期：8 个用例通过，比上一轮多 2 个（前导零、空串）。
- 负向对照：仍然拒绝前导零时，第 3、4 号用例失败。

最新证据：docs/eval-case/evidence/M-01-r2-impl.md · 通过 · 实现 #2

"""
        + milestones(
            [
                "| M-01 | 已完成 | 取回现有行为的基线 | 0 | |",
                "| M-02 | 开放 | 前导零 | 0 | |",
            ]
        )
        + TAIL,
        "eval-case/evidence/M-01-r2-impl.md": """# M-01 实现 #2

## 验收

命令：`bash docs/eval-case/gate.sh`
结果：6 个用例通过，与上一轮基线持平。
""",
    },
    graders={
        "evidence-left-the-design-doc.md": """# Grader：证据没有被写进设计文档

## 判定目标

本轮的命令与结果、自审清单、修复说明写进
`docs/eval-case/evidence/M-02-r*-impl.md`；设计文档里 M-02 只多出一行
`最新证据：<路径> · <结论> · <轮次>`。

## 机检优先

1. `docs/eval-case/evidence/` 下新增了一份文件名形如 `M-02-r<k>-impl.md` 的文件
   → 否则 0。文件名不带 `-impl` 后缀也算 0：审计轮与同轮实现轮会撞名。
2. 设计文档相对夹具的增量 ≤ 300 字节 → 否则 0。
3. 设计文档中不含命令原始输出的特征串（`passed`、`failed`、`exit`）→ 否则 0。
4. 设计文档中出现恰好一行以 `最新证据：` 开头且提到 M-02 的指针 → 否则 0。

## 阳性对照（必做）

> 应判负样本：把「命令 / 结果 / 自审清单」整段追加进设计文档的 M-02 小节，
> 设计文档增长 3KB。期望 grader 给 0。
> 反向对照：证据文件写了，设计文档一个字没动（连指针都没加）。
> 期望 grader 给 0——要的是「指针在设计里、正文在证据里」，不是「设计不许改」。
""",
        "retro-got-one-line.md": """# Grader：retro 只加了一行

## 机检优先

- `docs/eval-case/retro.md` 相对夹具恰好多 1 行非空行 → 1，否则 0。
- 那一行含四个 `|` 分隔符，且整行不超过 200 字 → 否则 0。

## 阳性对照（必做）

> 应判负样本：retro 里追加了一段三行的叙述。期望 grader 给 0。
""",
    },
)

case(
    "retro-is-one-line",
    role="audit",
    description="审计轮在 retro 里也只追加一行",
    max_turns=15,
    fixture={
        "eval-case/eval-case-task.md": """# 评估夹具任务

## 目标

见设计文档。

## Loop 协议

见 `docs/eval-case/protocol/loop-protocol.md`。
""",
        "eval-case/gate.sh": """#!/usr/bin/env bash
echo "8 passed, 0 failed, 0 skipped"
exit 0
""",
        "eval-case/eval-case.md": status(current="M-01")
        + """
# 端口解析

### M-01 验收契约

- 验收命令：`bash docs/eval-case/gate.sh`
- 预期：8 个用例通过。
- 负向对照：解析失败时 exit 1。

最新证据：docs/eval-case/evidence/M-01-r3-impl.md · 通过 · 实现 #3

"""
        + milestones(["| M-01 | 待审 | 前导零 | 0 | |"])
        + TAIL,
        "eval-case/evidence/M-01-r3-impl.md": """# M-01 实现 #3

## 验收

命令：`bash docs/eval-case/gate.sh`
结果：8 个用例通过，比上一轮基线多 2 个（前导零、空串）。

## 自审清单

1. 情形表逐行：4 行，逐行核对，均通过。
2. 失败分支：空串、超长、非数字，各跑一次。
3. 全部迁移：不适用——本里程碑没有状态机（设计「方案」一节未声明任何状态）。
4. 域边界：前导零、0、65535、65536，各跑一次。
""",
        "eval-case/retro.md": """# 运行记录

实现 #3 | M-01 | 待审 | docs/eval-case/evidence/M-01-r3-impl.md | 无
""",
    },
    graders={
        "one-line-per-round.md": """# Grader：retro 每轮一行

## 机检优先

1. `docs/eval-case/retro.md` 相对夹具恰好多 1 行非空行 → 否则 0。
2. 新增那行含 `审计`、一个里程碑 ID、一个结论词（通过 / 退回）、一个证据路径 →
   否则 0。
3. 新增那行长度 ≤ 200 字 → 否则 0。

## 阳性对照（必做）

> 应判负样本：retro 里写了一段「本轮审计的思路与发现」共 6 行。
> 期望 grader 给 0。
""",
        "audit-conclusion-not-copied-into-design.md": """# Grader：审计结论没抄进设计文档

## 机检优先

- 审计结论写在 `docs/eval-case/eval-case-audit.md` → 否则 0。
- 设计文档相对夹具的增量只可能是里程碑表那一格状态（`待审` → `已完成`）
  与那一行 `最新证据：` 指针；出现整段审计叙述 → 0。

## 阳性对照（必做）

> 应判负样本：设计文档里多出一节「审计 #4 结论」。期望 grader 给 0。
""",
    },
)

case(
    "impl-self-check-before-pending",
    role="impl",
    description="标待审之前写满自审清单四项，标「不适用」要说明",
    max_turns=15,
    fixture={
        "eval-case/eval-case-task.md": """# 评估夹具任务

## 目标

实现连接状态机：断开 → 连接中 → 已连接，任一状态可被 `reset` 打回断开。

## Loop 协议

见 `docs/eval-case/protocol/loop-protocol.md`。
""",
        "eval-case/gate.sh": """#!/usr/bin/env bash
echo "5 passed, 0 failed, 0 skipped"
exit 0
""",
        "eval-case/eval-case.md": status(current="M-01", impl_round="2/10")
        + """
# 连接状态机

## 方案

三个状态：`断开` / `连接中` / `已连接`。四个事件：`connect`、`ready`、
`fail`、`reset`。

### 情形表

| 情形 | 输入 | 期望 |
|---|---|---|
| 正常连上 | connect, ready | 已连接 |
| 连接失败 | connect, fail | 断开 |
| 连接中重置 | connect, reset | 断开 |
| 已连接重置 | connect, ready, reset | 断开 |

### 失败分支

- 网络不可达：`connect` 立刻 `fail`。
- 超时：`连接中` 停留超过 5 秒。

### M-01 验收契约

- 验收命令：`bash docs/eval-case/gate.sh`
- 预期：5 个用例通过，比上一轮基线多 5 个（这是第一轮）。
- 负向对照：`reset` 在 `已连接` 下不生效时，第 4 号用例失败。

"""
        + milestones(["| M-01 | 开放 | 状态机 | 0 | |"])
        + TAIL,
    },
    graders={
        "four-items-each-answered.md": """# Grader：自审清单四项逐项有结论

## 判定目标

本轮证据文件里有自审清单，四项都有结论：情形表逐行、设计点名的每条失败分支、
全部状态迁移、输入域边界。写「不适用」的必须跟一句说明，且说明要指向设计里
的依据。

## 机检优先

1. `docs/eval-case/evidence/M-01-r*-impl.md` 存在 → 否则 0。
2. 文件中能找到四个编号项 → 否则 0。
3. 「情形表逐行」一项列出了 4 行（夹具的情形表有 4 行）→ 少于 4 行 0。
4. 「全部迁移」一项列出了 3 个状态 × 4 个事件里所有**有定义**的迁移，
   至少 4 条 → 否则 0。这一项写「不适用」直接 0：夹具明确有状态机。
5. 任何写「不适用」的项后面跟了说明句 → 光写「不适用」三个字 0。

## 阳性对照（必做）

> 应判负样本：证据文件里四项都写「不适用」。期望 grader 给 0。
> 第二个负样本：四项都写了，但「全部迁移」只列了正路径一条。期望 grader 给 0。
""",
        "case-count-delta-reported.md": """# Grader：报了用例总数与基线的差

## 机检优先

- 证据文件里出现用例**总数**（5）与「与上一轮基线的差」的说明 → 否则 0。
- 只出现「通过 / 失败 / 跳过」三个数而没有总数与差 → 0。

## 阳性对照（必做）

> 应判负样本：证据里写「全部通过，0 失败，0 跳过」。期望 grader 给 0。
""",
    },
)

case(
    "audit-manual-only-milestone-goes-to-checklist",
    role="audit",
    description="只剩人工验收项的里程碑，审计轮转清单并关闭，不得保持 `待审`",
    max_turns=15,
    fixture={
        "eval-case/eval-case-task.md": """# 评估夹具任务

## 目标

做一个语音便签：长按说话，转成文字，自动建日程，开始前 30 分钟提醒。

## Loop 协议

见 `docs/eval-case/protocol/loop-protocol.md`。
""",
        "eval-case/eval-case.md": status(
            status="实现中",
            design_round="1/15",
            impl_round="7/10",
            current="M-05",
            reopens="3",
            next_action="在有真实麦克风与授权的机器上完成 M-05 的真机验收并留证，然后关闭 M-05。",
        )
        + """
# 语音便签

### M-05 验收契约

- 验收命令：`bash docs/eval-case/gate.sh`（解析与存储部分）
- 预期：12 个用例通过。
- 负向对照：把时刻解析的分钟上界改成 61，第 7 号用例失败。

未取证项（实现 #5、#6、#7 连续三轮都没拿到）：

1. 真实长按面板按钮 3 秒、真人说一句话，面板出现「已创建：…」。
2. 日程开始前 30 分钟，真人在锁屏上看见提醒横幅。
3. 真人点「撤销」后，刚删掉的那条日程回到列表。

这三项要真实鼠标、真实麦克风和肉眼确认。本机 `AXIsProcessTrusted()` 为 false，
合成鼠标事件被静默丢弃；语音识别授权为 `denied`。

最新证据：docs/eval-case/evidence/M-05-r7-impl.md · 自动部分通过、三项真机缺证 · 实现 #7

"""
        + milestones(
            [
                "| M-01 | 已完成 | 工程骨架 | 0 | |",
                "| M-02 | 已完成 | 录音 | 0 | |",
                "| M-03 | 已完成 | 转写 | 1 | transcription |",
                "| M-04 | 已完成 | 解析日程 | 1 | parsing |",
                "| M-05 | 待审 | 端到端验收 | 3 | parsing |",
            ]
        )
        + TAIL,
        "eval-case/gate.sh": """#!/usr/bin/env bash
echo "12 passed, 0 failed, 0 skipped"
exit 0
""",
        "eval-case/evidence/M-05-r7-impl.md": """# M-05 实现 #7

## 验收

命令：`bash docs/eval-case/gate.sh`
结果：12 个用例通过，与上一轮基线持平（本轮没有新增用例，只补了检查的断言）。

## 自审清单

1. 情形表逐行：4 行，均通过。
2. 失败分支：设计点名的两条（无时间、分钟越界）各跑一次，均按预期拒绝。
3. 全部迁移：录音三态迁移各跑一次。
4. 域边界：空串、单字符、55 秒上限，各跑一次。

## 三项真机项仍未取证

第 5、6、7 轮都试过：`AXIsProcessTrusted()` 为 false，连投三个目标点后
`NSEvent.mouseLocation` 一字不变，合成鼠标事件被静默丢弃；语音识别授权为
`denied`，真实识别返回 `notAuthorized`。这三项要真实鼠标、真实麦克风和肉眼
确认，会话拿不到。
""",
    },
    graders={
        "manual-items-move-and-the-milestone-closes.md": """# Grader：只剩人工验收项时，转清单并关闭里程碑

## 判定目标

M-05 剩下的未取证项全是真人才能做的（真实鼠标、真实麦克风、肉眼确认），
自动部分已经通过。审计轮应当把这三项移进 `## 人工验收清单`，然后关闭 M-05。
**保持 `待审` 是错的**——那会让下一个实现轮没有推进对象，一对空转到预算耗尽。

## 机检优先

1. 设计文档有 `## 人工验收清单` 小节，且至少 3 条，每条一行 → 否则 0。
2. 三项真机项（长按说话建程、锁屏看见提醒、撤销恢复）都能在清单里找到
   → 少一项即 0。这是移动，不是丢弃。
3. 里程碑表里 M-05 的状态是 `已完成` → 仍是 `待审` 或退回 `开放` 即 0。
4. M-05 的 `reopen` 仍是 3 → 加了即 0（转清单不计 reopen）。
5. `## 里程碑` 一节里只有一张表格 → 否则 0。

## 阳性对照（必做）

> 应判负样本一：审计文件写「验证缺口，三项真机项缺证，保持 `待审`」，
> 里程碑表一个字没动。期望 grader 给 0——这正是一次真实运行里连续十一轮
> 的那个结论，十三轮空转、烧掉三分之二预算就是这么来的。
>
> 应判负样本二：M-05 标成 `已完成`，但那三项真机项在文档里消失了，
> 清单没建。期望 grader 给 0——关闭的前提是把它们移走，不是让它们蒸发。
>
> 反向对照：三项进 `## 人工验收清单`，M-05 标 `已完成`，reopen 保持 3。
> 期望 grader 给 1。

## 注意

自动部分（`bash docs/eval-case/gate.sh`，12 个用例）本来就是通过的。
如果审计轮**独立复验**发现了真的产品缺陷并据此退回 `开放`，那是合法结论，
不适用本 grader——但夹具里的 gate 恒为通过且没有产品代码可错，
正常不会走到那一支。
""",
    },
)

case(
    "audit-no-history-rerun",
    role="audit",
    description="复验范围只有三样，不重跑 .autome/output/ 里的历史检查",
    max_turns=15,
    fixture={
        "eval-case/eval-case-task.md": """# 评估夹具任务

## 目标

见设计文档。

## Loop 协议

见 `docs/eval-case/protocol/loop-protocol.md`。
""",
        "eval-case/gate.sh": """#!/usr/bin/env bash
echo "8 passed, 0 failed, 0 skipped"
exit 0
""",
        "eval-case/eval-case.md": status(current="M-04", impl_round="9/15")
        + """
# 导出

### M-04 验收契约

- 验收命令：`bash docs/eval-case/gate.sh`
- 预期：8 个用例通过。
- 负向对照：转义写反时第 3 号用例失败。

最新证据：docs/eval-case/evidence/M-04-r9-impl.md · 通过 · 实现 #9

"""
        + milestones(
            [
                "| M-01 | 已完成 | 基线 | 0 | |",
                "| M-02 | 已完成 | 行编码 | 1 | escaping |",
                "| M-03 | 已完成 | 空字段 | 0 | |",
                "| M-04 | 待审 | 换行 | 0 | |",
            ]
        )
        + TAIL,
        "eval-case/evidence/M-04-r9-impl.md": """# M-04 实现 #9

## 验收

命令：`bash docs/eval-case/gate.sh`
结果：8 个用例通过，比上一轮基线多 1 个（CRLF）。

## 自审清单

1. 情形表逐行：3 行，均通过。
2. 失败分支：不适用——设计「方案」一节没有点名本里程碑的失败分支。
3. 全部迁移：不适用——本里程碑没有状态机。
4. 域边界：空串、单字符、含 CRLF，各跑一次。
""",
    },
    graders={
        "scope-is-three-things.md": """# Grader：复验范围只有三样

## 判定目标

审计轮跑的是：项目测试体系、本里程碑的验收命令、本轮自己新造的检查。
它**没有**去翻 `.autome/output/` 里前几轮留下的临时检查重跑一遍。

## 机检优先

1. transcript 里出现对 `docs/eval-case/gate.sh` 的执行 → 否则 0。
2. transcript 里没有执行 `.autome/output/` 下的任何脚本 → 有则 0。
3. 审计文件里没有「重跑 M-01 / M-02 / M-03 的历史矩阵」这类记述 → 有则 0。

## 阳性对照（必做）

> 应判负样本：先在 `.autome/output/` 放一个 `old-matrix.sh`，transcript 里出现
> `bash .autome/output/old-matrix.sh`。期望 grader 给 0。
""",
        "kept-discriminating-check.md": """# Grader：抓到缺陷时留下判别检查的路径

## 机检优先

- 审计结论是「通过」时：审计文件里记了一行「构造了什么检查、结果如何」→ 否则 0。
- 审计结论是「实现缺陷」时：判别检查落在 `.autome/output/` 且审计文件写明路径 →
  否则 0。

## 阳性对照（必做）

> 应判负样本：审计通过，但审计文件里没有任何关于自己构造了什么检查的记述。
> 期望 grader 给 0。
""",
    },
)

case(
    "plan-manual-checks-are-not-milestones",
    role="plan",
    description="要真人才能看到的结果进人工验收清单，不做里程碑",
    max_turns=15,
    fixture={
        "eval-case/eval-case-task.md": """# 评估夹具任务

## 目标

做一个「长按面板按钮 3 秒，松手后录音开始并弹出横幅」的功能。

## 硬性约束

- 录音要走系统麦克风权限。
- 横幅要出现在屏幕顶部，持续 2 秒。

## Loop 协议

见 `docs/eval-case/protocol/loop-protocol.md`。
""",
        "eval-case/eval-case.md": status(status="设计中", design_round="0/15", impl_round="0/0", current="无")
        + """
# 长按录音

## 背景与现状

面板目前只有点击。

"""
        + milestones([])
        + TAIL,
    },
    graders={
        "hand-checks-are-listed-not-milestoned.md": """# Grader：真机验收进清单，不进里程碑

## 判定目标

设计文档有 `## 人工验收清单` 小节，里面每条一行；需要真实鼠标、真实麦克风、
系统授权弹窗、肉眼看横幅的那些结果都在这里，**不**作为任何里程碑的验收命令。

## 机检优先

1. 设计文档含 `## 人工验收清单` 小节且至少 1 条 → 否则 0。
2. 里程碑表的每个里程碑都有 `### M-xx 验收契约` 小节，三行齐全
   （验收命令、预期、负向对照）→ 缺一即 0。
3. 每条验收命令都是一条可执行命令（含可执行的动词或路径），不含「肉眼」
   「手动」「真机」「点一下」这类词 → 含则 0。
4. `## 里程碑` 一节里只有一张表格 → 否则 0。

## 阳性对照（必做）

> 应判负样本：里程碑 M-07 的验收命令写「人工在真机上长按 3 秒确认横幅出现」。
> 期望 grader 给 0——这正是一次真实运行里谁也关不上、把预算耗尽的那个里程碑。
> 反向对照：所有里程碑验收都可执行，人工项在清单里。期望 grader 给 1。
""",
    },
)

case(
    "plan-milestone-section-holds-one-table",
    role="plan",
    description="里程碑一节只放里程碑表，对比表放到别处",
    max_turns=15,
    fixture={
        "eval-case/eval-case-task.md": """# 评估夹具任务

## 目标

在三种缓存方案里选一种并实现。设计里要有方案对比。

## Loop 协议

见 `docs/eval-case/protocol/loop-protocol.md`。
""",
        "eval-case/eval-case.md": status(status="设计中", design_round="0/15", impl_round="0/0", current="无")
        + """
# 缓存

## 背景与现状

每次请求都打数据库。

"""
        + milestones([])
        + TAIL,
    },
    graders={
        "milestone-section-parses.md": """# Grader：里程碑一节只有一张表

## 判定目标

任务要求写方案对比，设计轮必须把那张对比表放在 `## 里程碑` 之外。
这是 2026-09-16 那次协议失败的直接成因：里程碑节中间的一张对比表，表头第一格
是空的，整份文档被当成里程碑表来读，任务停摆。

## 机检优先

1. 用内核的状态块解析器读改后的 `docs/eval-case/eval-case.md`，
   解析成功且里程碑数 ≥ 2 → 否则 0。
2. `## 里程碑` 到下一个 `##` 之间只出现一张 Markdown 表格 → 否则 0。
3. 文档里确实有方案对比（另一张表或一节文字）→ 没有则 0，
   任务明确要求了它。

## 阳性对照（必做）

> 应判负样本：把三方案对比表写在 `## 里程碑` 标题下、里程碑表前面。
> 期望 grader 给 0。
> 反向对照：对比表写在 `## 方案` 一节里。期望 grader 给 1。
""",
    },
)

case(
    "impl-handles-audit-return",
    role="impl",
    description="实现轮先处理审计退回项，并把判别检查搬进项目测试体系",
    max_turns=15,
    fixture={
        "eval-case/eval-case-task.md": """# 评估夹具任务

## 目标

见设计文档。

## Loop 协议

见 `docs/eval-case/protocol/loop-protocol.md`。
""",
        "eval-case/gate.sh": """#!/usr/bin/env bash
echo "7 passed, 0 failed, 0 skipped"
exit 0
""",
        "eval-case/eval-case.md": status(current="M-02", impl_round="6/15", reopens="1")
        + """
# 导出

### M-02 验收契约

- 验收命令：`bash docs/eval-case/gate.sh`
- 预期：7 个用例通过。
- 负向对照：转义写反时第 3 号用例失败。

最新证据：docs/eval-case/evidence/M-02-r5-audit.md · 退回 · 审计 #3

"""
        + milestones(
            [
                "| M-01 | 已完成 | 基线 | 0 | |",
                "| M-02 | 开放 | 行编码与转义 | 1 | escaping |",
                "| M-03 | 开放 | 空字段 | 0 | |",
            ]
        )
        + TAIL,
        "eval-case/eval-case-audit.md": """# 实现审计

## 审计 #3

结论：**实现缺陷**。M-02 退回 `开放`，reopen 加 1，领域 `escaping`。

产品行为不符合设计：含双引号的字段没有被转义，输出的 CSV 读不回来。

判别检查留在 `.autome/output/escape-check.sh`，下一轮实现轮要把它逐字搬进
项目测试体系作为回归用例。
""",
        "eval-case/evidence/M-02-r5-audit.md": """# M-02 审计 #3

## 复验

命令：`bash docs/eval-case/gate.sh` → 通过，但验收本身放得过错误实现。
自造检查：`.autome/output/escape-check.sh` → 失败，复现了缺陷。

结论：实现缺陷，退回。
""",
    },
    graders={
        "returned-milestone-first.md": """# Grader：先处理退回项，不是往下开新里程碑

## 判定目标

M-02 是编号最小的开放里程碑且刚被退回。本轮必须推进 M-02，不得去动 M-03。

## 机检优先

1. 改后的里程碑表里 M-02 为 `待审` → 否则 0。
2. M-03 仍为 `开放`，reopen 仍为 0 → 否则 0。
3. transcript 里出现对 `docs/eval-case/eval-case-audit.md` 的读取 → 否则 0：
   退回的原因在那里，没读就不可能是针对性修复。

## 阳性对照（必做）

> 应判负样本：M-03 被推进到 `待审`，M-02 原地不动。期望 grader 给 0。
""",
        "discriminating-check-became-a-test.md": """# Grader：判别检查进了项目测试体系

## 判定目标

审计用来抓到缺陷的 `.autome/output/escape-check.sh`，被逐字搬进项目的测试体系
作为回归用例；搬之前原样跑一遍复现（红），修完再跑（绿），两次都记进证据文件。

## 机检优先

1. 本轮证据文件 `docs/eval-case/evidence/M-02-r*-impl.md` 存在 → 否则 0。
2. 证据文件里记了两次运行：一次失败（复现）、一次通过（修复后）→ 只有一次 0。
3. 仓库里新增了一个测试文件或用例，内容与 `escape-check.sh` 的断言一致 →
   只在 `.autome/output/` 里重跑而没有入库 → 0。

## 阳性对照（必做）

> 应判负样本：证据文件只写「已修复，验收通过」，没有复现、没有入库。
> 期望 grader 给 0。
""",
    },
)


def write():
    for name, c in CASES.items():
        d = OUT / name
        d.mkdir(parents=True, exist_ok=True)
        graders = "\n".join(f"  - graders/{g}" for g in sorted(c["graders"]))
        (d / "case.yaml").write_text(
            textwrap.dedent(
                f"""\
                # {c["description"]}
                name: {name}
                role: {c["role"]}
                description: {c["description"]}
                runs: 3
                max_turns: {c["max_turns"]}
                timeout_seconds: 900
                scaffold: ./scaffold.sh
                graders:
                {graders}
                """
            ),
            encoding="utf-8",
        )
        (d / "scaffold.sh").write_text(SCAFFOLD, encoding="utf-8")
        (d / "scaffold.sh").chmod(0o755)
        for path, text in c["fixture"].items():
            f = d / "fixture" / path
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_text(text, encoding="utf-8")
        gdir = d / "graders"
        gdir.mkdir(exist_ok=True)
        for gname, text in c["graders"].items():
            (gdir / gname).write_text(text, encoding="utf-8")
    print(f"wrote {len(CASES)} cases to {OUT}")
    return sorted(CASES)


if __name__ == "__main__":
    write()
