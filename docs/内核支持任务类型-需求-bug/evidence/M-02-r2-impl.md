# M-02 · 实现 #2

## 范围与修复说明

实现设计 §3：task.create 校验 kind/brief/draft，按 UTF-8 字节限制正文，支持从正文派生 request；任务与正文沿用 store.create_task 事务写入。草稿不 tick、不占槽位；task.start 经状态机、排队取号、task.started 事件再 tick。task_json 增加 kind；草稿复盘明确拒绝。未改旧测试、协议、CLI、apps。

上一轮审计 M-01 通过，无退回缺陷，无需搬入审计缺陷复现检查。新增的立即开始检查最初因测试仓库没有初始提交而失败（invalid reference: main），补齐隔离仓库初始提交与 main 分支后通过；未削弱断言。

用户裁定：B-01/B-03/B-04 忽略、不实现；B-02 纳入 M-06，B-05（N1-01）纳入 M-07，均已写明范围、验收与负向对照。本轮仅推进最小开放项 M-02；新增项保持开放，避免违反一轮一个里程碑。

## 命令与结果

- 基线 `cargo test -p automed --lib dispatch::tests::task_card`：0 个，560 filtered out；`.autome/output/m02-baseline.txt`。
- 验收 `cargo test -p automed --lib dispatch::tests::task_card`：12 个全部通过，相对上一轮 0 增加 12，来源是设计点名的 12 个新用例；无删除、无忽略。最终原始输出 `.autome/output/m02-tests-final.txt`。
- `cargo test -p automed --lib`：572 个全部通过，相对上一轮 560 增加 12；`.autome/output/m02-lib-final.txt`。首次运行因上述新夹具缺少初始提交为 571/572，补齐后全量重跑通过。
- `cargo test -p autome-domain task_card`：7 个全部通过，与上一轮 7 相同，差 0；验证全部 Draft 触发器及非 Draft Start 拒绝。`.autome/output/m02-domain.txt`。
- `cargo clippy -p automed --lib --tests -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`：通过；前两条输出在 `.autome/output/m02-clippy.txt`、`m02-fmt-check.txt`。
- 本轮没有跑 workspace 全量、CLI/e2e、protocol eval；它们按设计由后续里程碑验收。本轮不是 M-05 全量门禁的通过证据。

## 标待审之前的自审清单

### 1. 情形表逐行

设计 §3、§7 Q1/Q3/Q5/Q6 没有独立情形表，按参数与行为落法逐项核对：

| 情形 | 检查与结果 |
|---|---|
| 不传新参数 | create_without_the_new_params_is_unchanged：feature、queued、位置 1、返回三字段、无正文行、task.created 存储载荷恰为 project_id，通过 |
| bug / null kind | create_accepts_kind_bug_and_task_get_and_project_get_carry_it：task.get/project.get 为 bug，null 默认 feature，通过；dashboard 共用 task_json |
| 未知 kind / 参数类型错 | create_rejects_an_unknown_kind_in_chinese：大写、空串、数字、对象；draft null/字符串/数字，brief null/数字均拒绝，任务数不变，通过 |
| 正文边界 | a_brief_of_exactly_64_kib_is_accepted_and_one_byte_more_is_refused：中文 UTF-8 恰 65536 原样存储，65537 拒绝且任务数不变，通过 |
| 空正文 | an_empty_brief_is_refused：空串、换行制表符、全角空格均拒绝，通过 |
| 仅正文 | a_brief_without_a_request_takes_its_request_from_the_first_heading：首个一级标题、无一级标题退到首非空行、单字符、无有效标题、request 类型错误，通过 |
| request 和正文同时给 | with_both_the_request_names_the_task_and_the_brief_is_kept_verbatim：request 决定 title/slug；正文含 CRLF、中文、占位符与尾部空白逐字保留，通过 |
| 草稿创建 | a_draft_is_created_without_starting_or_a_queue_position_and_is_not_on_the_dashboard：draft、不启动、位置 null、槽位 0、两张看板空、无 worktree，通过 |
| 开始 | task_start_queues_a_draft_behind_the_parallel_limit_and_records_task_started：满槽排在已有队列后为位置 2，task.started 存储载荷正确；另一个有初始提交的真实 Git 夹具在空槽立即 active，通过 |
| 非草稿开始 | task_start_refuses_a_task_that_is_not_a_draft：RPC 拒绝，通过；其余状态由 domain 7 用例穷尽验证 |
| 草稿取消 | a_draft_is_cancelled_and_archived_without_a_worktree：cancelled、归档、无 worktree、正文保留，通过 |
| 复盘与读白名单 | a_draft_is_refused_a_retro_and_task_start_is_not_a_read_method：中文拒绝复盘，task.start 不在 READ_METHODS，通过 |

### 2. 失败分支

本里程碑点名的非法类型、空正文、超限正文、缺少标题、非草稿 Start、草稿 Retro 均在上表执行并通过。拒绝创建同时断言任务数不变。授权拒绝、读盘失败、超时、外部服务不可达：不适用，本里程碑未引入这些调用或设计失败分支；正文文件落盘与 Git 提交重试由 M-03/M-07 验收。

### 3. 全部迁移

新建→Draft、默认新建→Queued、Draft→Queued（槽满）、Draft→Queued→Active（空槽）、Draft→Cancelled 并归档均已实跑。domain task_card 的 7 个用例额外验证每个非 Draft 状态拒绝 Start、Draft 拒绝其余所有 Trigger，并检查线形、槽位与终结属性。草稿 Retro 拒绝保持原有任务，不进入会话。

### 4. 输入域边界

已跑 kind 缺省/null/feature/bug/未知值；draft 缺省/true/非法类型；brief 空串/各类空白/单字符/65536/65537 UTF-8 字节；标题缺省、首个一级标题、非一级标题回退、仅 # 与空格；正文含占位符、CRLF 和尾部空白。日期、闰日、跨年、前导零：不适用，本里程碑没有日期或数字字符串输入域。

## 负向对照

按 M-02 验收契约逐项临时注入并还原：65536 判断改成 >=；草稿仍以 Queued 入账；task.created 存储载荷新增 kind。对应边界、草稿与兼容用例均失败，原始输出为 `.autome/output/m02-negative-{size,draft,event}.txt`。还原后重跑 12 个验收用例全部通过；负向变更未保留。
