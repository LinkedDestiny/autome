# M-02 实现 #2

## 修改与验收

仅在 technical-design §8.2、§9、§14 追加任务卡、Draft、配置与 IPC 说明及源码符号出处；原有行未改写。未修改源码、README、测试或协议。上一轮 M-01 审计通过，无退回检查需要搬入测试体系。

验收命令：`sed -n '/^~~~sh check-docs$/,/^~~~$/p' docs/文档补齐-bug-loop-任务卡-待开始/文档补齐-bug-loop-任务卡-待开始.md | sed '1d;$d' | sh -s M-02`。

- 修改前：`M-02 FAIL 103/131`，退出 1，原始输出 `.autome/output/m02-before.txt`。
- 修改后：`M-02 PASS 131/131`，退出 0，原始输出 `.autome/output/m02-after.txt`。
- 用例总数 131，比上一轮 M-01 的 103 增加 **28**：8.2 位置 1、§8 关键词 17、§9 关键词 4、§14 关键词 6。同一 M-02 命令修改前后总数差 0，通过数增加 28。没有删减或改写检查。
- `git diff --check` 通过；目标文档仅新增 20 行，删除 0 行。

## 标待审之前的自审清单

### 1. 情形表逐行

逐行读取下列源码及其测试，与新增文本核对，均一致；D7 同时检查所有 73 个代码锚点存在。

| 设计行 | 检查与结果 |
|---|---|
| T-40 | task_create 的 kind 缺省 feature、解析 feature/bug、未知值拒绝，与文档一致。 |
| T-41 | task_create 的 as_str、trim、len > 65536；类型、空白与字节上限均写明。 |
| T-42 | 标题先找 # 行，再回退第一非空行；缺标题拒绝、request 优先、正文保真均核对。 |
| T-43 | create 的 draft 分支不 tick，Draft 与 false/null 返回值由现有测试覆盖。 |
| T-44 | create 仅非草稿 bug 查 audit；apply_for 在 Start 查 audit，与文字一致。 |
| T-45 | store::create_task 写 task_briefs，task_brief 读原字符串，未做 trim。 |
| T-46 | task_card_read 测试覆盖 get 原文、无正文 null、归档可读、列表省略。 |
| T-47 | task_create 对附件 is_file 检查及「附件不存在」错误核对。 |
| T-48 | write_task_inputs 的 first_time 附件复制与 dest.exists 正文防覆盖分支核对。 |
| T-49 | write_task_inputs 的 commit_paths 与 inputs 提交消息核对，现有 scheduler 测试验证 Git 内的字节。 |
| T-50 | task_start 先 Start、mark_queued，再事件、tick、三个返回字段，核对一致。 |
| T-51 | a_draft_is_cancelled_and_archived_without_a_worktree 覆盖直接取消归档且无检出。 |
| T-52 | parse_project 的整数过滤 1..=20 与错误原文、默认 3 核对一致。 |
| T-53 | DEFAULT_FIX_ROUNDS = 3；resolve 使用 project.bug.unwrap_or_default，不从全局继承。 |
| T-54 | dispatch 的 task.reply 按 uses_bug_loop 分流；空信息拒绝；需求任务构造 Reject 原样传 info。 |
| T-55 | task_start 写命令、READ_METHODS 排除由现有测试验证。 |
| T-56 | task_create 读取 kind、brief、draft，§14 指回 §8.2。 |

### 2. 失败分支

通过现有 automed/domain 单元测试检查：未知 kind、非字符串 brief、空/全空白 brief、超过 65536 字节、缺标题、非 Draft 开始、审计角色关闭、Bug reply 空白/非等待节点、非法 fix_rounds。错误文字另逐字对照 task_create、create、task_start、dispatch、apply_for、parse_project。

附件不存在的拒绝分支按源码 `is_file` 与错误字符串核对；本任务不修改运行行为，未另造真实附件失败场景。文件落盘失败由现有 `a_card_without_attachments_is_committed_byte_for_byte_in_the_inputs_commit` 的目录被文件阻挡分支覆盖，Git 提交失败与保留已有正文由 `an_existing_card_is_committed_after_git_add_failed` 覆盖。授权拒绝、外部服务不可达、超时：不适用，本里程碑新增的是本地文档与本地任务输入说明，未引入这些外部调用分支。

### 3. 全部迁移

本里程碑迁移由现有 domain 测试覆盖：Draft + Start → Queued；所有非 Draft + Start 拒绝；Draft + Cancel → Cancelled；Draft 其它事件拒绝。automed 的 task_card 测试覆盖排队、实际开始、二次开始拒绝、取消与归档。Bug 的 await_info/not_a_bug + 非空 Reply 均回 locate；遍历 Node::BUG 和 Node::ALL，空串、全空白与非等待节点均拒；需求 Reply 与 Reject 等价由 dispatch 测试覆盖。§5.6 其余迁移属已关闭 M-01，文档未改，domain 全量测试一并复验。

### 4. 输入域边界

现有测试实际覆盖正文 65536 字节接受、65537 拒绝（含多字节中文）、空串/ASCII 与 Unicode 空白、单字符标题、无 # 标题回退；fix_rounds 1/3/20 接受，0/-1/21/u32 边界/浮点/布尔/字符串/数组拒绝。kind 空串、大写、非字符串拒绝。前导零、闰日、跨年不适用：本里程碑没有日期输入或字符串数字转换契约。same_failure_limit 与复现退出码未改，属 M-01/M-03 范围。

## 测试执行

命令 `cargo test -p autome-domain --lib > .autome/output/m02-domain.txt 2>&1`：退出 0，384 个通过，0 失败、0 忽略。

本轮未运行全量 `sh scripts/ci-local.sh`，按设计由宿主门禁执行，M-03 再显式执行；不将本轮单元测试结果宣称为全量门禁。

命令 `cargo test -p automed --lib > .autome/output/m02-automed.txt 2>&1`：退出 0，648 个通过，0 失败、0 忽略。两组共 1032 个测试。上一轮证据没有记录这两个单独 lib 的计数，不能把全量门禁数直接作为子集基线；本轮未增删任何 Rust 测试，代码与上一轮一致。可直接比较的文档验收基线及增加 28 项的来源见上文。

自审完成后将 M-02 标为待审；M-03 留开放。无阻塞。
