# M-01 · 实现 #1

结论：M-01 验收通过，交独立审计；本轮只推进 M-01。上一轮无审计，没有需要逐字移植的审计检查。

## 实现与修复

- 新增 TaskKind（feature 默认、bug）、Draft 独立线上状态、Start 转换与拒绝规则。
- v5 单事务迁移新增 kind、queued_seq、task_briefs；任务和正文同事务保存，正文逐字节读取。
- 排队按 NULL 旧行的创建时间、非 NULL 行的入队序号排序；Requeued 保留位置。
- 新增设计列出的 7 个领域测试、5 个 store 测试。既有测试只补中性 kind 字段和版本断言，不改既有行为断言。
- 编译所需的最小联动：dispatch 的生产构造暂用 Feature；scheduler 的穷尽 trigger_name 增加 Start → start。RPC 功能未提前实现。
- 首次编辑出现语法/穷尽匹配编译错误，已修复。随后队列用例红灯：排序编辑误落到 tasks_with_metrics；已恢复该函数，把排序放回 queued_tasks。红灯记录 `.autome/output/m01-queue-red.txt`，最终同一检查绿灯见验收日志。

## 命令与结果

全部测试使用项目已有内存/隔离数据库，未运行新二进制访问真实账本。

| 命令 | 用例总数及结果 | 基线差与来源 | 原始日志 |
|---|---|---|---|
| `cargo test -p autome-domain task_card` | 7 通过，0 失败/忽略 | 0 → 7，新增领域测试 | `.autome/output/m01-acceptance-final.txt` |
| `cargo test -p automed --lib store::tests::task_card` | 5 通过，0 失败/忽略 | 0 → 5，新增账本测试 | 同上 |
| `cargo test -p automed --lib store::tests::requeued_old_ledger_rows_and_new_rows_roundtrip` | 1 通过，0 失败/忽略 | 1 → 1，无新增 | 同上 |
| `cargo test -p autome-domain --lib` | 306 通过，0 失败/忽略 | 299 → 306，新增 7 | `.autome/output/m01-domain-all.txt` |
| `cargo test -p automed --lib store::tests` | 48 通过，0 失败/忽略 | 43 → 48，新增 5 | `.autome/output/m01-store-all.txt` |
| `cargo clippy --all-targets -- -D warnings` | 退出 0 | 编译全部目标 | `.autome/output/m01-clippy.txt` |
| `cargo fmt --all -- --check` | 退出 0 | 格式检查 | `.autome/output/m01-fmt-check.txt` |

验收三条总计 13，较设计基线 1 增加 12。扩大检查的 354 个用例包括这 13 个，不重复计新增。未跑全 workspace/e2e（M-05 全量验收）。首次所谓 baseline 日志是在生产修改后、新测试加入前生成，store 当时编译失败，不作为基线通过证据；基线采用设计已记录数字及既有测试计数。

## 标待审之前的自审清单

### 1. 情形表逐行

- §1 类型：两种值的 parse/serde/as_str 往返与默认 Feature 均通过，未知值拒绝。
- §1 线上形状：Draft 精确为 `{"state":"draft"}`，Start 为 `{"trigger":"start"}`；与 Queued 区别明确。
- §1 分类：node=None、不占槽、不等待用户、不终结，逐项断言通过。
- §2 迁移：Queued、Requeued、Active、Paused、Stopped、Failed、Done、Cancelled 八种旧状态逐条保持，kind=Feature、queued_seq=NULL、正文不存在、版本=5。
- §2 读写：两种 kind 经 get/list/unfinished 读取；正文含中文、emoji、CRLF、空白、`{request}` 原样往返；无正文返回 None。
- §2 排队：空队列、旧行逆序插入、新任务、早创建草稿、同秒时间戳、两草稿反创建顺序开始、Requeued 保序均通过；草稿不进队列且不占槽。
- §2 计数：空集合及单 Draft 的独立桶通过。

### 2. 失败分支

- 非草稿 Start：八种其它状态逐条拒绝，中文理由精确匹配。
- Draft 的其它触发器全部拒绝，Pause 精确匹配「任务尚未开始」。
- 未知类型（空串、大小写变体、前后空格、未知词）parse/serde 均拒绝。
- 正文写入失败：SQLite BEFORE INSERT 触发器 RAISE(ABORT) 注入失败，create_task 返回错误且任务行回滚、任务总数不变。
- 拒绝授权、超时、外部服务不可达：不适用，本里程碑没有授权或外部服务调用。真实读盘失败不适用，本里程碑未引入读文件路径；沿用已有 store 错误传播。

### 3. 全部迁移

- Draft × Start → Queued + None；Draft × Cancel → Cancelled + RunCancel。
- Draft × SlotAvailable/Pause/Stop/Resume/Approve/Reject/Merge/ExtendBudget/RerunFrom/SessionEnded/CoreStepDone/GateDone：12 种触发器逐一拒绝。
- 其余八种状态 × Start：逐一拒绝。
- v4 → v5 覆盖全部八种旧状态；现有重复迁移、新版本拒绝等 store 测试通过。既有状态迁移由领域全量 306 用例复验。

### 4. 输入域边界

- TaskKind 空串、大小写、空白、未知值与两个合法端点均验证。
- 队列空集合、同秒时间、较早创建草稿、反向开始顺序；计数单元素；正文 Unicode/模板字样/换行/尾空白均验证。
- 64KiB 上限属于 M-02 RPC 校验，账本层只负责原样保存，本轮不适用。日期闰日、前导零、跨年解析不适用：本轮没有日期解析输入域。

## 负向对照

- Draft 序列化临时改为 Queued：指定线上形状测试失败，恢复后通过（`.autome/output/m01-negative-draft-wire.txt`）。
- v5 临时移除 DEFAULT feature：旧账本迁移测试失败，恢复后通过（`.autome/output/m01-negative-migration-default.txt`）。
- queued_tasks 仍按 created_at 排序时，排队顺序断言实际失败；修正后同一用例通过（上述 queue-red 与最终验收日志）。
- 所有临时负向修改已恢复。
