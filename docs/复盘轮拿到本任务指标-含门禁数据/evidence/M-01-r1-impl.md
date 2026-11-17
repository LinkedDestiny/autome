# M-01 · 实现 #1

范围：仅 M-01（A-2 数据计量、A-3 旧指标兼容）。起点 `bc371c9a11e4c0a47e3ffbe92125f429de648e34`；没有上一轮审计退回、没有需要搬入的审计检查。未改协议、schema、其他 worktree、真实账本或 M-02/M-03 实现。

## 实现

- TaskMetrics 追加五个 Option 字段和五个指标名；缺失不序列化，value 返回 None；实测零返回 Some(0.0)。
- Store::timed_events 按主体与事件种类过滤、按 seq 排序，用 SQLite 解析秒数，非法时间保留 None。
- gate_counts 按 node 统计结果，按最近的同 node/log 起始事件算耗时；未配对、不完整时间不推测耗时。
- measure_task 只计算，write_task_metrics 保持最终指标写入和事件发布；复盘启动接入留给 M-02。

## 验收命令与数量

按契约顺序运行（各段输出分别保存）：

`cargo test -p autome-domain --lib metrics:: && cargo test -p automed --lib gate_metrics && cargo test -p automed --lib task_metrics && cargo test -p automed --lib version_page`

| 过滤器 | 用例总数 | 上一轮设计基线 | 差与来源 | 结果 |
|---|---:|---:|---|---|
| metrics:: | 18 | 16 | +2：D1、D2 | 全过，0 失败/忽略 |
| gate_metrics | 9 | 0 | +9：T1–T6、S1、V1、C1 | 全过，0 失败/忽略 |
| task_metrics | 19 | 13 | +6：T1–T6 | 全过，0 失败/忽略 |
| version_page | 12 | 11 | +1：V1 | 全过，0 失败/忽略 |

共 58 次执行（基线 40，+18）；T1–T6 与 V1 被两个过滤器重复执行，去重后 51 个独立用例（基线 40，新增 11）。这是首个实现轮，基线采用设计提供的数量，未伪称本轮实跑旧提交。原始输出：`.autome/output/m01-{domain,gates,task,version}.txt`。最终故障注入恢复后四段再跑，均绿。

`cargo fmt --all -- --check`、`git diff --check` 通过。按本轮用户限制，不额外主动运行无过滤的 lib 全量或项目全量门禁；指定快检自行选择的测试另记。未运行 `sh scripts/ci-local.sh`。

## 标待审之前的自审清单

### 1. 情形表逐行（A-Q4）

- gate/fix_gate result：T1 按节点算 2 次，kind 分别 full/repro，绿。
- merge_gate result：T1 算 1 次，T2 失败算 1，绿。
- repro_check/main_check：T1 不入两组，T5 各计 10 秒，绿。
- reused/skipped：T3 不增加次数和耗时；C1 经 Store 写入后仍是每轮 3 次失败 1、合并前 2 次失败 1，绿。
- 未配对 started（消失/HEAD 移动/运行中在指标输入上同形）：T4 全零，C1 混入后不影响结果，绿。
- 熔断回放：生产 record_gate_result 的已有结果直接返回分支不 append_event；指标只读取已落账事件，无新的账本行就不新增计数。未改变该路径。
- 耗时配对：T5 同 log 跨节点不配、同节点不同 log 不配、重复 start 取最近一条；五类节点总计 50 秒，绿。
- 无配对 result：T4 仍计运行，但耗时零，绿。
- 负时间差与任一侧时间缺失：T5 均零，绿；S1 非法日期解析 None，绿。
- 排队：T5 放入更早的 gate.queued 仍为 50 秒；生产 gate.started 写于 start_with_passes 成功之后，Store 只取 started/result，绿。
- 空事件：T6 五项 Some(0)，绿。

### 2. 失败分支

T2 用数值退出码 gate::TIMED_OUT（124）覆盖 ok=false 超时；正常失败在 C1 覆盖。S1 覆盖非法时间，T5 覆盖缺失时间，T4 覆盖结果无起始。D1/V1 覆盖旧 JSON 缺字段。空输入由 T6/S1 覆盖。授权拒绝、外部服务不可达不适用：本里程碑是本地账本读取和纯函数，不新增授权或外部调用。复盘读取失败回退属于 M-02，本轮未接入。

### 3. 全部迁移

计量输入：无事件→零；仅开始→未知不计；开始→成功/失败结果→计次数及时间；开始→沿用/跳过→不计；多次开始→结果→最近起点；结果无开始→只计次数，分别由 T1–T6 覆盖。持久化：C1 验证 measure_task 前后 tasks.metrics 为空、task.metrics 事件数为零；随后 write_task_metrics 写入相同指标且仅发一次事件。未新增任务状态迁移，自动/手动复盘通道属于 M-02。

### 4. 输入域边界

D1：缺失；D2/T6：零；T4：单事件与无匹配；T5：不同节点/日志、重复开始、负差、缺时间；S1：ISO 日期与 SQLite datetime 格式、非法日期、空 kinds、其他任务隔离、seq 顺序，均绿。前导零、闰日、跨年等文本业务输入不适用：本层不新增日期文本解析器，使用 SQLite strftime；计数输入域为账本事件序列。

## 负向对照（临时修改后全部恢复）

1. 五个 value 分支改为 unwrap_or(0)：`cargo test -p autome-domain --lib gate_metrics_old_json`、`cargo test -p automed --lib gate_metrics_old_json` 各 1 例失败（退出 101），分别抓到 None 变零、均值 2 变 1。
2. gate.reused 当作 result：`cargo test -p automed --lib gate_metrics_reused` 1 例失败（退出 101）。C1 的真实查询本来就排除 reused，因此该单点纯函数注入由 T3 判别，不能声称 C1 也因此变红。
3. 改按 kind=quick 分组：`cargo test -p automed --lib gate_metrics_group_by_node` 1 例失败（退出 101）。

四次单例执行都是预期红，原始输出 `.autome/output/m01-negative-*.txt`，汇总 `.autome/output/m01-mutation-summary.txt`。恢复后验收四段均绿，不把预期红计入最终失败数。

## 使用方核对

`rg -n 'task.metrics|METRIC_NAMES|TaskMetrics' crates apps/web/app apps/web/server` 完整结果在 `.autome/output/m01-usage.txt`，按使用方归类核对：

- domain metrics：本轮实现，D1/D2；lesson：通过 is_metric 校验，词表自动接受新增名。
- store：Option 反序列化兼容；仍只有 write_task_metrics 持久化，S1/C1；tasks_with_metrics 与相关历史查询不改。
- scheduler/dispatch：最终写入入口签名保持；measure_now 条件不变；C1 验证只算不写。复盘仍读旧存量是 M-02 的待办。
- version_page：rows/realized 都 filter_map(value)，V1 验证缺失不进均值。
- dispatch_protocol：直接发布 METRIC_NAMES，无需改动。
- dispatch_curation（补查）：移除实验用 filter_map(value)；curation::metric_for 仍选旧名字，不影响。
- launcher/protocol eval_run：vocabulary 由词表生成；表格新行留 M-02。eval 占位符规则无需改动。
- guards/lib：状态快照与模块引用，无新字段读取。
- Web task-stage-model：仅读旧轮次字段，本轮不增加事件触发点，无 UI 改动。
- end_to_end/prompt_rendering：既有 TaskMetrics 构造使用 Default，兼容新增字段；相关新增行为测试留 M-02。

## 指定快检

命令：`sh scripts/ci-quick.sh --since bc371c9a11e4c0a47e3ffbe92125f429de648e34`。

最终版本退出 0：8 步执行、0 沿用、0 失败，64 秒；相关测试阶段 47 秒，预算 120 秒。原始输出：`.autome/output/m01-quick-final.txt`。Rust format/clippy、Web lint/typecheck 均通过。快检自动选择的测试是 domain lib 489、automed lib 1053、CLI bins 67，共 1609 例，0 失败/忽略。相对起点的推算基线 1598 增加 11（domain +2、automed +9、CLI 不变），与新增独立用例一致；基线不是本轮重跑旧提交得到的数值。

第一次同命令也退出 0（151 秒），输出在 `.autome/output/m01-quick.txt`；该次 Web 首次安装后 lint/typecheck 显示输入变化而不存通过记录，是尚未实现的 M-03 范围，因此安装完成、故障注入还原后再跑最终快检。没有在测试里使用真实账本。

## 交接

四项自审及验收通过后，M-01 标为待审；状态块 implementation-round 为 1/15，其他里程碑保持开放。状态字段、表格结构和不得标已完成的边界已检查；retro 只追加一行。提交范围只含五个 Rust 文件与本轮三份任务记录，等待独立审计，不启动下一会话。
