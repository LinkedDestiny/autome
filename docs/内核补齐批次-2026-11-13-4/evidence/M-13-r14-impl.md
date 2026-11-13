# M-13 · 实现 #14

范围：只推进 M-13（F-R3/R4/R5/R7，F-V3–V6），implementation-round 为 14/80。上一轮 M-12 审计通过，没有退回缺陷，无需搬入上一轮审计的判别检查。

## 实现与修复

- Git 同时读取 `diff -M --name-status -z` 与 `--numstat -z`，按原始路径合并。四种 status、改名 from、顶层与仓库 deleted_count；二进制计数保持 0，特殊文件名不被拆开。
- 分支清理后验证记录的合并提交有第二父提交，再用第一父提交到合并提交读取。工作区从 merge.repo 恢复成员清单与各自提交。补充的 workspace 用例先抓到清理后只剩文档仓库，修复后再验；没有顺带改变 scheduler 的工作仓库推导。
- events.since 的 subject 使用 subject_id 索引，先过滤再分页，latest_seq 仍为全账本；省略 subject 走原路径。非字符串拒绝、空字符串可查询空结果，超大 after_seq 饱和到 SQLite 整数上界，避免转换成负数后重新返回旧事件。
- 收割先记录 usage 再写 session.ended；显式停止也解析已有 usage 后写同一载荷，使用同一个 ended_at。按 subject/session_id 去重。正常、非零退出、失踪、超时、停止分别映射；未知指标为 null。CLI JSON 透传、人读不新增输出，浏览器读网关仍拒绝。
- 正常退出测试最初错误地等重试，实际缺产物会协议失败，已改为等可观察的结束事件。短命 CLI 可能先退出、后写 started_at，现有墙钟函数对负时长返回 None；断言区分这个 null、有效墙钟值与显式 usage 指标，不伪造 0。
- 负向脚本初次还原时因 Ruby 字符串别名漏还原停止调用，最终复验抓到 stopped 事件数为 0；已恢复调用，最终证据仅以还原后整条验收为准。

## 命令、计数与基线

1. 原样验收：`cargo test -p automed -- task_changes_status_ events_by_subject_ session_ended_ && cargo test -p autome-cli session_ended_ && npm ci --prefix apps/web && npm run test --prefix apps/web -- test/read-gate.test.ts`。日志分别为 `.autome/output/m13-restored-final.txt`、`m13-cli-final.txt`、`m13-npm-ci-final.txt`、`m13-web-final.txt`。automed 库 6 + e2e 7，CLI 1，Web 16，合计 **30 个**。
2. 相关既有测试：`cargo test -p automed --lib -- events_since_ change_summary_counts_commits_files_and_lines`，**5 个通过**，日志 `.autome/output/m13-related.txt`。
3. 格式化用 `cargo fmt --all`；差异检查 `git diff --check`。未执行全量门禁和宿主快检，fmt check / 全项目 clippy / npm lint / typecheck 留给宿主。

计数：本轮去重总数 **35**，不把负向注入和重跑重复计数。轮初同一选择范围已有 Web 15 + 相关旧测试 5 = 20，本轮新增 automed 13 + CLI 1 + Web 1 = **+15**。设计原预计 automed 11，本轮额外增加失踪/指标边界库用例 1、工作区清理 e2e 1，因此验收从预计 28 变成 30，没有删除或跳过既有用例。上一轮 M-12 主验收 9，本轮 30，差 **+21**；上一轮去重 12，本轮 35，差 **+23**，来自里程碑选择范围变化，不能当作全项目用例增长。

## 负向对照

- 把两条 diff 的 `-M` 换成 `--no-renames`：`cargo test -p automed --lib -- task_changes_status_all_types_and_unusual_paths events_by_subject_filters_before_pagination_and_validates_type` 中改名断言变红。
- 同次注入把新查询的 subject_id 换成 `json_extract(payload, '$.task_id')`：上述命令中过滤/分页断言变红；合计 2 个失败、退出 101，日志 `.autome/output/m13-negative-lib.txt`。
- 去掉 stop_running_session 的 record_session_ended：`cargo test -p automed --test end_to_end session_ended_e2e_stopped`，1 个失败、退出 101，日志 `.autome/output/m13-negative-stop.txt`。最终调用已恢复并统一停止时间戳。

## 标待审之前的自审清单

### 1. 情形表逐行

- added/deleted/modified/renamed 四行：库测试与单仓库 e2e 逐个检查 path/status/行数/from；普通项四键、改名项五键钉死，deleted_count=1。
- 分支存在 → 合并/删除分支：e2e 断言文件清单、增删行数、deleted_count 不变，merged=true；缺合并提交/单亲提交返回 available=false。
- 工作区：清理前后两个成员的清单相同，分别从各自 merge.repo 提交读取。
- subject 省略、指定任务、指定项目：省略序列化字节相同；两任务交替、项目事件混入时，一条一页不丢不重，全局 latest_seq 不变。过滤不信任 payload.task_id。
- 退出 0 / 退出 3 / stop / timeout：四条真实假 CLI 流程各一条 ended；Vanished 的独立检查映射 crashed/null。角色、轮次、退出码、时长、费用、7 个载荷键逐项断言；再次 tick 不重复。
- CLI JSON 包含完整载荷；人读不输出；浏览器带 subject 仍拒绝。

### 2. 设计点名的失败分支

- 浏览器拒绝授权：read-gate 新用例验证不放行，原有 15 个测试全保留。
- Git 缺失引用、无合并提交和非双亲提交：库/e2e 分别检查错误或 available=false。
- 超时与崩溃：真实子进程 timeout/nonzero 测试；stop 与 timeout 检查子进程不存活。失踪、坏时间戳、缺失指标分别为 crashed/null，不宣称零费用。
- 空输入：空 subject、空 diff、limit=0 均检查空结果；非法 subject 类型拒绝。
- 外部服务不可达：不适用，本里程碑不调用外部服务。

### 3. 全部状态迁移

- Running → Exited(0)/Exited(nonzero)/Killed/TimedOut/Vanished 全部覆盖；Running 不生成结束事件；同一结束重复写入保持一条。
- 分支存在 → 合并 → Cleanup/分支不存在：单仓库和工作区各跑一次；合并记录失效 → unavailable 跑一次。
- 事件查询无过滤 → 任务过滤/项目过滤、游标首条 → 后续页 → 空页全部覆盖。

### 4. 输入域边界

- 路径：中文、制表符、换行、改名、二进制、单文件、空 diff。
- 分页：after_seq=0、单元素页、末页空结果、limit=0、超过上限限制为 1000、after_seq=u64::MAX；空 subject、未知 subject、非字符串。
- 指标：None、非空费用 0.25、精确时长 123ms；无效时间、结束早于开始、跨年相差 1000ms。
- 前导零与闰日：不适用，本 API 没有接收数字字符串或日期输入；时间来自内部会话记录。

## 边界

测试仅使用临时账本、临时仓库和已有假 CLI；未运行真实 autome/automed，没有修改页面、协议副本或其它 worktree。最终提交与验收结论见下。

最终结论：原样验收命令退出 0，30/30 通过；相关测试 5/5 通过，失败 0、忽略 0。三项负向对照已还原，git diff --check 通过。四项自审执行完毕，M-13 申请待审，状态块 14/80；本轮提交到当前分支，不推送。
