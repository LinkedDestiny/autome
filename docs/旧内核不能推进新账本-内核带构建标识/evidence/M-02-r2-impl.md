# M-02 实现 #2：账本记录与旧内核只读

结论：验收通过，申请待审。implementation-round 为 2/35。本轮仅推进 M-02；上轮审计 M-01 通过，无退回缺陷及须逐字迁入的审计检查。

## 实现

- v8 追加 kernel_mark 单行表，旧迁移块不改；BEGIN IMMEDIATE 将读取、比较、按需更新合为事务。同标识不更新 updated_at、不发事件。
- kernel_guard 复用 build_id 比较与唯一拒绝文案。Ctx 身份默认取编译常量，测试直接注入；生产环境开关留给 M-03。
- tick 与全部非 READ_METHODS 写命令先只读检查，再取得驱动锁、锁内 claim，持锁至全部副作用完成；锁 I/O 或记录读取失败均停止推进。
- tick 保留布尔 skipped、新增 older_kernel；写命令使用 older_kernel 错误码；env.get 给出双方身份与可推进性。
- 新增 7 个真实文件账本 e2e 与 3 个库测试；迁移测试重新打开 v7 文件并验证队列与序号保留。

## 命令与结果

1. `cargo test -p automed --test end_to_end older_kernel && cargo test -p automed --lib kernel_mark`：7 + 3 = 10 例通过，0 失败、0 忽略。原始输出 `.autome/output/m02-e2e.txt`、`m02-store.txt`。
2. `cargo test -p automed --lib store::tests::deps_store_migration_preserves_queue`：1 例通过。
3. `cargo test -p automed --lib driver_lock::tests`：5 例通过，含锁释放、继承文件描述符、等待期限与通知次数。
4. `cargo test -p automed --lib build_id`：15 例通过。2–4 原始输出 `.autome/output/m02-regression.txt`。
5. 已用 rustfmt 格式化所改 Rust 文件；`git diff --check` 通过。未运行全量门禁或宿主快检。

本轮最终执行 **31 个不同用例**（验收 10 + 直接相关回归 21），全部通过。上一轮 M-01 基线为 21，本轮净差 **+10**：保留 build_id 15；未重跑上轮 build_rerun 4 与 CLI 版本 2（-6）；新增本里程碑 10，并运行已有驱动锁 5、v6 迁移 1（+16）。不同筛选集合的差不代表全量覆盖增加 10。项目测试实际新增 10：e2e 从 183 到 190，库从 876 到 879；筛掉的用例不是已执行或跳过。

开发中的编译错误（误用未依赖的 chrono/tempfile）及测试夹具的枚举 JSON/响应路径错误均已修正；最终命令在最终代码上通过。上轮没有实现缺陷，因此审计要求的红绿复现不适用。

## 标待审之前的自审清单

### 1. 情形表逐行

`kernel_mark_claim_matrix` 实跑每行，并验证最终标记、updated_at 哨兵及事件数为零：

| 设计情形 | 检查结果 |
|---|---|
| 无记录 | git 与 unknown 都写入当前值 |
| 文本相同 | git 与 unknown 都放行、不写时间 |
| git 祖先数更大 | 更新为当前 |
| git 祖先数更小 | 拒绝、原记录不变 |
| 祖先数相同、sha 不同 | 拒绝、不写 |
| 当前 git、记录 unknown | 更新为 git |
| 当前 unknown、记录 git | 拒绝、不写 |
| 两个不同 unknown | 拒绝、不写 |
| 记录解析失败 | 空串与未来格式均拒绝、不写 |

### 2. 失败分支

- 旧内核拒绝：tick、7 种写方法同一句话；持有另一把锁时旧写命令仍直接拒绝；3 种读方法和 env.get 正常。
- 锁文件打不开：用目录占据锁文件，最新身份也 skipped / internal，标记、事件、recovered 均不变。
- 记录读取失败：删除 kernel_mark 后，tick 与写命令拒绝推进，无事件与恢复副作用。
- claim 遇 RESERVED 锁：另一连接未提交新标记时返回忙，提交后旧身份被拒，未凭旧快照写入。
- 驱动锁忙：tick skipped 且 older_kernel 为 null；等待超时由已有 driver_lock 用例实跑。
- 外部服务不可达：不适用，本里程碑保护路径不调用外部服务。授权拒绝不是新的身份认证逻辑，本轮对应构建身份拒绝已实跑。

### 3. 全部迁移

- 无标记→当前、更旧标记→更新身份、unknown→git、相同→不写、较新/不可比/损坏→拒绝：矩阵全部通过；逃生口降低标记属于 M-03，不在本轮实现。
- v7→v8 重新打开：空标记、原队列与队列序号保留；已有 v6→当前迁移测试也通过。
- 两条推进通道 tick / 写命令：锁外拒绝、锁错误拒绝均实跑；锁内比较的事务竞争由 RESERVED 锁测试覆盖。
- 旧内核连续 3 tick：排队任务、运行中会话、事件序号原样，recovered=false，没有 worktree/协议副本/会话启动。重复同身份 tick 无事件且 updated_at 哨兵不变。
- 未知占位符正负对照：Requeued Implement 任务协议含未知占位符，旧身份 3 tick 状态不变；同身份 tick 将其判成包含“占位符”的配置失败，证明故障现场可达。

### 4. 输入域边界

- 标记矩阵实跑空记录、空字符串、未知格式、单行记录、零计数、前导零、9999999 最大计数、闰日与跨年时间文本。
- build_id 的 15 例回归验证解析、比较与 UTC 边界；本轮不改变标识解析域。
- 表约束 id=1，claim 只访问该行；写方法以原 READ_METHODS 排除集统一保护，未改常量内容。

## 用户裁定与范围

B-01、B-02、B-04、B-05 忽略、不做，已写 retro。B-03 纳入新开放里程碑 M-07，给出隔离 home/prefix、清理、错误传播及 ci-config 测试验收契约；按最小开放编号和每轮一个里程碑，本轮不提前实现 M-07。未触碰真实账本、安装路径或其它 worktree。
