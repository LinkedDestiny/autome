# M-03 实现证据（实现 #3，预算 3/15）

## 范围与修复说明

仅追加 README 的命令行说明、示例和 `[bug]` 配置；technical-design 本轮不改。已有 `[setup]` 与 `same_failure_limit` 核对后原样保留。标题回退说明沿用设计 T-42 与 Backlog B-08 的授权；没有增加新行为。上一轮审计通过，无退回缺陷、无须搬运审计回归检查。未启动下一会话，未操作真实账本。

## 验收命令与基线

- 文档命令：`sed -n '/^~~~sh check-docs$/,/^~~~$/p' docs/文档补齐-bug-loop-任务卡-待开始/文档补齐-bug-loop-任务卡-待开始.md | sed '1d;$d' | sh -s M-03`。
- 修改前 `M-03 FAIL 138/155`，修改后 `M-03 PASS 155/155`，检查退出码 0；原始输出分别为 `.autome/output/m03-before.txt`、`m03-docs.txt`。这是需求补齐前后的红绿，不是审计缺陷回归。
- 本轮文档用例总数 155，较上一轮 M-02 的 131 增加 24：README 命令行 15、Host gate 9；同一 M-03 检查修改前后总数差 0，通过数增加 17。没有删改检查。
- 全量命令：`sh scripts/ci-local.sh > .autome/output/m03-ci.txt 2>&1`，单独保存退出码至 `.autome/output/m03-ci.exit`，不以 tail 的退出码代替门禁结果。
- 上一轮基线取 `.autome/output/audit-ci.txt`，已保留副本 `.autome/output/m03-baseline-ci.txt`：Rust 1201（1200 通过、1 忽略），Desktop 166，Web 301。Rust 通过数为 25+14+384+648+106+16+7=1200；忽略的基准用例也计入总数。

## 标待审之前的自审清单

### 1. 情形表逐行

已逐行打开代码上下文，README 对照结果如下；D7 同时检查所有 73 个源码锚点。

| 行 | 检查与结果 |
|---|---|
| R-01 | main.rs 传递 kind，dispatch.rs 缺省 Feature；吻合。 |
| R-02 | bug_loop.rs apply_for 的定位、复现、修复、门禁、复核、rebase、merge 尾段与审计角色守卫；吻合。 |
| R-03 | main.rs 文件/stdin 分支读取字节后 String::from_utf8；吻合。 |
| R-04 | dispatch.rs task_create 的标题、trim 空卡与 len > 65536；吻合。标题回退对应 T-42，未误写为字符上限。 |
| R-05 | main.rs 非 JSON 草稿提示，task.rs Draft 不占槽位；吻合。 |
| R-06 | main.rs print_json 原样输出，dispatch 创建返回 task/started/queue_position；吻合。 |
| R-07 | task.rs Start 只允许 Draft；吻合。 |
| R-08 | bug_loop.rs 两个等待节点接受非空 Reply；dispatch.rs 非 Bug Loop 转 Reject；吻合。 |
| R-09 | apply_for 失败/停止重跑四个节点，熔断分支接受同一门禁；吻合。 |
| R-10 | CLI 轮数解析和 apply_for 正数/溢出检查，追加修复预算；吻合。 |
| R-11 | CLI 现有 task_card 测试使用 stdin、bug、draft、json 建卡并启动；示例吻合。 |
| R-12 | config_io.rs parse_project 1..=20、config.rs DEFAULT_FIX_ROUNDS=3；吻合。 |
| R-13 | start_fix 检查预算，手动 RerunFrom 直接 session；README 明确手动重跑绕过预算检查，吻合。 |
| R-14 | scheduler.rs run_gate 对 FixGate/MergeGate 拼装复现与项目门禁；无项目门禁只跑复现，吻合。 |
| V-01 | DEFAULT_GATE_SAME_FAILURE_LIMIT=3；原文正确，保留。 |
| V-02 | parse_project 的 1..=10；原文正确，保留。 |
| V-03 | gate_failure.rs counts 只含 Gate/MergeGate/FixGate；保留。 |
| V-04 | streak 的 !p.tripped 守卫保证熔断后计数从 1 开始；保留。 |
| V-05 | DEFAULT_SETUP_TIMEOUT_MINUTES=30；setup.rs ensure/skip_workspace 与领域 fingerprint/needs_run 对应 Preparation 的缓存、超时、workspace 跳过、日志规则；保留。 |

### 2. 失败分支

用本轮全量项目测试覆盖，不新增实现镜像测试：

- 非 UTF-8、空卡、超长卡、读盘失败：CLI `task_card_a_brief_from_a_file_names_the_task_and_an_oversized_one_is_refused`，检查错误文本、退出码与建卡数量不变。
- 非草稿启动：`task_card_start_refuses_a_task_that_is_not_a_draft`；缺标题：`a_brief_without_a_request_takes_its_request_from_the_first_heading`。
- 空 reply / 非等待节点、非法重跑节点、修复预算耗尽：`bug_loop_reply_is_refused_when_empty_or_not_waiting`、`bug_loop_rerun_from_accepts_locate_fix_review_and_rebase_only`、`bug_loop_fix_budget_runs_out_after_the_configured_rounds`。
- 无法复现、126/127、超时：`bug_loop_timeout_and_unrunnable_commands_do_not_count_as_reproduced` 与 `bug_loop_second_consecutive_miss_waits_for_info`。
- setup 失败及配置非法：全量中的 setup 与 config_io 用例；原文未改。
- 拒绝授权、外部服务不可达：不适用，本里程碑只补本地文档，没有授权或外部服务调用路径。

### 3. 全部迁移

README 所述迁移逐项对照 apply_for，并由现有全量测试执行：Draft→Queued（start）；Queued→Locate；Locate→ReproCheck/AwaitInfo/NotABug；两个等待节点 Reply→Locate；复现成功→Fix、未中→Locate/第二次→AwaitInfo；Fix→FixGate；FixGate 通过→FixReview、失败→Fix；FixReview 通过→Rebase、打回→Fix；Rebase 成功→MergeGate、冲突→Fix；MergeGate 失败→Fix、通过→共享合并尾段；预算耗尽→Failed、extend→Fix；失败/停止→四种合法 rerun 节点；同因熔断→原门禁 rerun/extend；feature reply→Reject。对应 bug_loop.rs 的 `bug_loop_*`、`same_failure_bug_rerun_and_extend` 和 CLI 的 `task_card_reply_rejects_the_design_with_the_text_verbatim` 测试。未改状态机，也没有引入新通道。

### 4. 输入域边界

本轮全量执行：config_io 的 `bug_fix_rounds_are_read_from_the_bug_section` 检查 1/3/20，非法值检查 0/-1/21/大整数/浮点/字符串/布尔/数组；`same_failure_config_bounds` 检查 1/10，invalid 检查 0/11 等；dispatch 的 `a_brief_of_exactly_64_kib_is_accepted_and_one_byte_more_is_refused` 检查 65536/65537 字节（含多字节汉字），空卡检查空串/空白/全角空白；Bug 复现测试检查退出码 0/124/126/127。追加轮数检查正数、零与溢出。日期、前导零日期、闰日、跨年不适用：本里程碑没有日期输入域。

## 最终结果

全量门禁退出码 **1**，耗时 418 秒。Format、Clippy、Rust、Contract、Web lint/typecheck/build 通过；Desktop 与 Web test 失败。M-03 保持开放，不能标待审。

| 测试体系 | 上一轮总数 | 本轮总数 | 差 | 本轮结果与解释 |
|---|---:|---:|---:|---|
| Rust | 1201 | 1201 | 0 | 1200 通过、1 忽略，与基线相同。 |
| Desktop | 166 | 105 | -61 | 104 通过、1 失败；Electron DOM harness 在读取 61 个子用例前失败，整层未执行。 |
| Web | 301 | 301 | 0 | 271 通过、11 失败、19 跳过；4 个测试文件失败、29 个通过。 |

Desktop 报 dom-harness.js exited null，无 stderr；额外用 Node spawnSync 对同一 Electron 二进制运行 --version（10 秒限时）得到 status:null、signal:SIGABRT，最小启动亦失败，未取得 DOM 验收。Web 明确报 listen EPERM: operation not permitted 127.0.0.1，属于当前执行环境端口权限阻塞。日志为 .autome/output/m03-ci.txt，Desktop 摘录 m03-desktop.txt。未修改测试、未跳过失败测试、未改配置绕过门禁，也未请求不可用的提权。

上面自审列出的 Rust/CLI 测试均已随全量通过；Desktop/Web 未通过项不能算验收完成。下一轮首先在允许 Electron 启动与本地监听的环境重跑原始门禁，再按基线核对全部用例。H-01 英文措辞人工确认仍留合并前处理。此为环境阻塞，不是协议失败。
