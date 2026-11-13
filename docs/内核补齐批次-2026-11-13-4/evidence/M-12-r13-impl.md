# M-12 实现证据（实现 #13）

结论：验收通过，申请待审。implementation-round 为 13/80，只推进 M-12。上一轮审计 #11 对 M-11 判定通过，没有退回缺陷，也没有需逐字搬入的审计判别检查。

## 实现范围

- 补全任务卡、Bug 文档和复盘教训，按设计顺序返回；证据文件按名称排序，仅列文件，保留既有字段并添加 source。
- 任务文档目录存在时使用 worktree；目录消失时使用项目主检出（工作区为文档仓库）。默认分支通过 Git ls-tree 确定成员，过滤主检出中不存在的文件；状态块通过 git show 读取默认分支内容。未合并、仓库不可读或文档不存在时保持空清单 / null。
- 需求状态块仍使用 StatusBlock，Bug 文档使用 BugDoc。CLI 显示合并后的里程碑及 Bug 文档绝对路径，不为 Bug 打印缺少字段的需求轮次。
- 技术设计 §14 补齐字段和来源契约。未改面板、协议、账本 schema、调度状态机或门禁脚本。

## 命令、结果与计数

1. 原样验收：`cargo test -p automed -- task_documents_ && cargo test -p autome-cli task_documents_`，退出 0。automed 库 5 + end_to_end 3 + CLI 1 = **9 个用例**，全部通过，0 失败、0 忽略。最终日志：`.autome/output/m12-acceptance.txt`。
2. 相关旧用例：`cargo test -p automed --lib -- the_document_list_includes_the_per_round_evidence_files a_task_without_an_evidence_directory_still_lists_its_documents && cargo test -p automed --test end_to_end bug_loop_task_get_reports_the_fix_round_for_bugs_only`，退出 0，**3 个用例**通过。日志：`.autome/output/m12-related.txt`。
3. `cargo fmt --all` 完成格式化；`git diff --check` 通过。未运行全量门禁、宿主快检（fmt check / 全项目 clippy / npm lint / typecheck）；由宿主执行。
4. 首次试跑修正了两处测试夹具问题：macOS /var 与 /private/var 别名改为 canonicalize 比较；CLI 创建 Bug 使用现有 task new --kind bug 语法。没有削弱产品行为断言。

计数基线：轮初 HEAD 211f84a9 在主验收的 task_documents_ 过滤范围内没有用例，本轮新增 9 个，符合设计预期 9 个。相关旧用例 3 个未增减，本轮去重总数 **12**，相对同范围轮初 3 个为 **+9**。上一轮 M-11 主验收为 47 个，本轮主验收 9 个，数值差 **-38**；来源是里程碑过滤范围从门禁车道切换到文档读取，不是删除或跳过 38 个测试。与上一轮去重总数 76 相比本轮 12 为 -64，同样为不同测试范围，不能解释成覆盖退化。

## 负向对照

- 强制始终选 worktree：运行 `cargo test -p automed --lib task_documents_after_cleanup_come_from_the_default_branch`，退出 101，1 个失败（清单为空，预期 3）。日志：`.autome/output/m12-negative-fallback.txt`。
- 交换文档列表前两项：运行 `cargo test -p automed --lib task_documents_lists_card_bug_doc_and_lessons_in_order`，退出 101，1 个失败（标签顺序不符）。日志：`.autome/output/m12-negative-order.txt`。
- 两项注入均完整还原，随后原样主验收 9 个全绿；负向对照是重复执行已有用例，不计入新增数或最终通过数。

## 标待审之前的自审清单

### 1. 情形表逐行

- 任务卡 → 需求设计 / Bug 文档 → 任务文件 → 评审 → 裁决 → 审计 → 运行记录 → 复盘教训 → 证据：order 单测逐项断言；真实需求流程、Bug 流程分别验证文档类型，均通过。
- worktree 文档目录存在：carry_source_worktree 验证单文件及空目录，source 为 worktree，空目录不错误回退，通过。
- 合并清理后：after_cleanup 单测与 merged_task e2e 验证 source、绝对路径、终态里程碑；默认分支与磁盘内容不同时仍读提交内的状态，通过。
- 工作区：workspace_reads_the_docs_repo_default_branch 验证读取文档成员而不是工作区根目录，通过。
- 已取消 / 未合并对应的缺失文档情形：cancelled_unmerged 单测在默认分支无目标文档、主检出只有未跟踪卡片时得到空清单 / null，通过。
- 复盘产物：retro_adds_lessons 真实运行复盘，文档列表出现 lessons.md，通过。
- CLI：真实需求任务合并、清理后显示 M-01 已完成；Bug 显示可定位的绝对文档路径，通过。

### 2. 失败分支

- 文档不存在、默认分支没有文档、仓库已删除 / Git 读取失败：cancelled_unmerged 返回空清单 / null，通过。
- 状态块格式无效：carry_source_worktree 返回解析错误；默认分支路径也验证无效文档返回 Unreadable，通过。
- 默认分支有文件而磁盘已删除：default_branch 单测排除 lessons.md；未跟踪 retro.md 不泄漏进清单，通过。
- 授权拒绝、外部服务不可达、超时：不适用，本里程碑没有新增授权、网络或超时分支，复用现有本地 Git 执行器。

### 3. 全部迁移

- 文档源 worktree → default_branch：需求与 Bug 的真实合并 / Cleanup 流程都跑过，合并前后断言通过。
- 有文档 → 文档删除 / 仓库删除：单测验证返回空列表、null，不返回残留内容，通过。
- 本轮不修改任务状态机；源选择每次请求重新计算，非持久状态，无额外状态迁移。需求和 Bug 两类解析通道、单仓库和工作区两类根路径均已覆盖。

### 4. 输入域边界

- 空清单、单文件、证据目录缺失、非 Markdown 证据、名为 .md 的目录：新增及既有相关单测通过。
- 自定义文档根含中文和空格、证据乱序创建、磁盘与默认分支内容不一致：单测 / e2e 通过。
- 前导零、闰日、跨年、数值最大值：不适用，本里程碑没有数字或日期输入域，也不新增 slug 校验规则。

## 限制与交接

- 设计 Backlog B-07 保持原样：absolute 指向主检出磁盘文件，未提交内容可能与 git show 状态块不同；本轮按已定契约实施。
- 所有流程测试使用独立临时账本与仓库，测试夹具在结束时清理；未接触真实账本。证据不代表宿主全量门禁已经通过。
