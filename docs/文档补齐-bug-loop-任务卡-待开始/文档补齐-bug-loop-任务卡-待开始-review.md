# 设计评审 #1

评审对象：`文档补齐-bug-loop-任务卡-待开始.md`，design-round 为 `0/15`；检出 HEAD 为 `0a3d79148f04eeabc08759c247503353e836ee72`。

结论：发现 **1 条属于协议六类的问题**，提交裁决轮处理；另有 2 条非阻塞建议。未修改设计文档、状态块、里程碑状态或实现代码。本轮不增加 design-round，不启动下一会话。

## 问题

### D1-P01：修复轮预算被描述成所有启动路径的硬上限，与手动重跑的实际行为不符

- **类别：违反任务目标（第 4 类）。**
- **追溯：**任务 §3 要求 3、§4 验收 2、§5 约束 1/3（逐条与代码一致；代码有缺陷时按实际行为写，不在本任务修代码）；设计 D4 的 T-27、D5 的 R-13；影响 M-01 的预算说明与 M-03 的 README 配置说明。
- **问题位置：**T-27 规定「已开轮数达到上限时下一次要开修复轮即 `Failed(FixBudget)`」，R-13 也将「用完任务停下」作为未限定路径的描述。D2 又要求实现轮不得改动既有行的陈述，因此不能把这个事实修正留给实现轮自行改写。
- **源码证据：**`crates/autome-domain/src/bug_loop.rs` 的 `start_fix`（约 47–60 行）确实检查 `progress.fix_round >= limit`；但 `apply_for` 的 `Failed / Stopped + RerunFrom` 分支（约 148–162 行）对 `Node::Fix` 直接调用 `session(BugStep::Fix, None, ctx)`，没有调用 `start_fix`。`crates/automed/src/scheduler.rs` 的 `advance` 使用这条转换，`perform` 的 `Action::StartBugSession` 分支仍会为它追加 `bug.fix_round_started`，不会因为是手动重跑而免计数。
- **具体反例：**审计及实现角色开启，Bug Loop 已启动，任务因 `FixBudget { limit: 1 }` 失败，已开修复轮数为 20；执行 `rerun <task> fix` 时领域转换仍返回 `Active(Fix)` 和启动修复会话动作，而非 T-27 断言的 `Failed(FixBudget)`。现有测试 `bug_loop::tests::bug_loop_rerun_from_accepts_locate_fix_review_and_rebase_only` 就覆盖了这个超预算状态，并断言重跑成功。
- **根因与同类检查范围：**把正常流转使用的 `start_fix` 检查推广到了全部修复启动路径。本轮检查了 T-23..T-33、R-09/R-10/R-13 以及 `apply_for` 的正常返工、追加预算、手动重跑分支。T-23/T-24/T-25/T-26 的正常返工经过 `start_fix`；T-28 的追加预算和 T-31/R-09 的手动重跑直接启动会话。因此需要同步限定 T-27、R-13 的适用范围，不能只改一个落点。
- **建议裁决落点：**T-27/R-13 改为正常自动转换在开启下一修复轮前检查预算，并如实说明手动 `rerun … fix` 不经该检查、仍计入已开修复轮数。保留 T-28 的实际追加预算语义。若把绕过上限视为代码缺陷，按任务约束记入设计 Backlog，本任务不修代码。无需新增实现测试或扩大交付范围。

## 非阻塞建议

以下不单独触发复审，由裁决轮决定是否纳入 Backlog。

- **S-01：README 标题规则可与 T-42 对齐。**R-04 仅提第一个 `# ` 标题，T-42 则已完整记录没有该标题时取第一非空行的回退。`dispatch.rs::task_create` 及测试 `a_brief_without_a_request_takes_its_request_from_the_first_heading` 明确接受 `brief = "x"` 和 `"## 次级标题\n正文"`。README 可加一句回退说明；当前未明确写成「必须有一级标题」，所以不据此提出阻塞问题。
- **S-02：M-03 验收命令可保留失败退出码。**目前 `echo`、`tail` 位于 `&&` 链之后，整条 shell 命令最终状态可能来自 `tail`，而非文档检查或 CI。建议显式保存并返回检查退出码，避免将来仅看进程退出码的调用方误判。现有预期已要求核对 `M-03 PASS 154/154`、`ci-local exit 0` 和各步骤结果，因此不把此建议包装为「验收不明确」或「里程碑不可执行」。

## 独立复核与边界

- 已读取简报、任务文件、固定协议、项目规则和完整设计；以当前源码核对 Bug Loop 转换、复现/修复/合并门禁拼装、任务卡建卡与落盘、reply/start、预算与重跑。
- `git diff cf522fb HEAD -- crates README.md docs/development/technical-design.md` 无差异，本轮引用的代码及目标文档与任务基线一致。
- M-01、M-02、M-03 各有独立 `### M-xx 验收契约`，均包含「验收命令、预期、负向对照」三行；未据验收明确程度提出问题。
- 实跑 D7 文档检查三个参数，分别得到 `M-01 FAIL 76/102`、`M-02 FAIL 76/130`、`M-03 FAIL 83/154`，与设计记录一致。这是未实现文档的预期红色基线，不是实现验收通过，也不是协议失败。72 个源码锚点均通过字面检查，字面命中不替代语义审查。
- 定向运行现有测试：`cargo test --offline -p autome-domain --lib bug_loop::tests::bug_loop_rerun_from_accepts_locate_fix_review_and_rebase_only -- --exact`，1 个通过、0 个失败、383 个被筛选；没有新增测试，用例数增量为 0。该测试验证的是代码允许超预算手动重跑，支持 D1-P01。首次用未限定模块名的 `--exact` 筛选命中 0 个用例，未作为通过证据；随后改用上述完整名称取证。
- 本轮未运行全量 `sh scripts/ci-local.sh`，未运行连接真实账本的 `autome` / `automed`。原始检查输出保存在 `.autome/output/review-1-*.log`。
- 设计状态块七个必需字段齐全，单仓库未写 `repos:`；`## 里程碑` 节只有固定表格。本轮保持其原样。
