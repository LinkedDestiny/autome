# M-03 实现 #3：显式接管

结论：本里程碑验收 4 例通过，申请待审。implementation-round: 3/35。上轮审计 M-02 通过，无退回缺陷，因此逐字迁入审计检查及其红绿复现不适用。

## 实现与范围

- Ctx 的 allow_older_kernel / adopted_older_kernel 默认 false；Ctx::new 不读取环境。
- 只读预检在尚可接管时放行至驱动锁；锁内 BEGIN IMMEDIATE 比较、降低标记、追加 kernel.adopted 同事务完成。提交成功后才消耗接管机会；失败保持原标记。
- 事件记录 from/to/via/method。新内核再次推进正常抬回记录，同一旧进程之后被拒，避免反复降级。
- entry::apply_kernel_env 接入 automed main 与 CLI Client::open；版本输出复用同一身份选择。仅字面量 1 开启逃生口；测试身份覆盖只编译进 debug，拒绝非法、新于当前及不可比身份，并在 stderr 警告。
- autome core 尚未存在，由 M-04 创建入口时调用同一 helper；本轮没有提前推进 M-04。README 属于 M-06。
- 全部场景使用项目既有临时文件账本，不接触真实 AUTOME_HOME。

## 命令与结果

- 验收：`cargo test -p automed --test end_to_end kernel_adopt && cargo test -p automed --lib kernel_adopt`：3 + 1 = 4 例通过；输出 m03-e2e.txt、m03-env.txt。
- 相关回归：`cargo test -p automed --test end_to_end older_kernel`：7 例；`cargo test -p automed --lib kernel_mark`：3 例；`cargo test -p automed --lib adoption_event_failure`：1 例；`cargo test -p automed --lib build_id`：15 例，均通过；输出 m03-older.txt、m03-mark.txt、m03-atomic.txt、m03-build.txt。
- `cargo check -p autome-cli` 通过（m03-cli.txt）。
- `cargo test -p autome-cli --test cli build_id`：2 例通过（m03-cli-version.txt）。
- 上述原始输出均在 .autome/output/。已用 rustfmt 格式化修改文件，git diff --check 通过。未运行全量门禁或宿主快检。
- 最终不同用例共 **32**，全部通过，0 失败、0 忽略。上一轮实现基线 31，本轮差 **+1**：保留旧内核 7、kernel_mark 3、build_id 15；未重跑驱动锁 5 和迁移 1（-6），新增本轮 5 例并补跑 CLI 版本 2（+7）。这不是全量测试数量的比较。项目新增 5 例：e2e 190→193，库 879→881。
- 开发中测试误引用 Reply.error，首次编译失败；改用现有 outcome 序列化断言后通过。不是产品缺陷红绿证明。

## 标待审之前的自审清单

### 1. 情形表逐行

- 显式旧内核接管：kernel_adopt_advances_and_records_event 验证排队任务离开 Queued、标记下降、事件恰好一条且 payload 四字段精确相等。
- 写命令通道：同例通过 task.create 接管，method=task.create，事件仍恰好一条。
- 同一旧内核重复推进：kernel_adopt_new_kernel_restores_mark_without_event 连续 tick 两次，事件不增加。
- 新旧交替三次：kernel_adopt_only_once_despite_alternating_kernels 第一轮成功，后两轮 skipped 且有 older_kernel，写命令也拒绝，事件总数 1。
- 新内核接回：记录恢复 GUARD_NEW，无新增接管事件，不消耗新内核的接管标记。
- 未开启逃生口：7 个 older_kernel 回归保持拒绝、只读、无副作用行为。

### 2. 失败分支

- 非法开关与标识：kernel_adopt_env_is_explicit 逐项验证，只有 1 开启；空/非法/未来/不可比覆盖被忽略。
- 事件写失败：adoption_event_failure_rolls_back_mark 用 SQLite BEFORE INSERT 触发器强制 ABORT，claim 返回错误，标记仍为新值且事件数 0；去除故障重试成功、标记变旧、事件 1 条。
- 账本读取失败、锁文件 I/O 失败：older_kernel_lock_error_stops_all_advancing 回归通过，推进被阻止。
- SQLite RESERVED 锁：kernel_mark 回归保持忙时不按旧读数更新。
- 外部服务不可达、外部授权、网络超时：不适用，接管事务不调用外部服务；身份拒绝已覆盖。

### 3. 全部迁移

- 新标记→旧标记（tick 与写命令）；旧标记→相同（重复 tick）；旧标记→新标记（新内核接回）；接管已消耗且标记再次变新→拒绝（tick 与写命令），均由三个 e2e 实跑。
- adopted=false→true 只在成功提交后发生；失败事务回滚由强制事件失败测试覆盖。已消耗标志不复位，由三轮交替断言覆盖。
- 无记录、相同、更旧、更新、unknown/不可比/损坏记录的基础比较与不写矩阵，由 kernel_mark 3 例回归覆盖。
- 进程重启会创建新 Ctx，默认 adopted=false；本轮未增加其它状态机或迁移版本。

### 4. 输入域边界

- 开关：未设置、空串、0、true、01、前导空格、尾换行均无效，精确 1 有效。
- 覆盖标识：未设置、空串、非法、相同、严格更旧、9999999 最大计数/未来日期、同计数不同 sha 均检查。
- 既有 build_id 15 例覆盖前导零、时间格式、闰日与跨年；本轮不改变解析域。
- 接管次数边界：0→1，重复以及三轮新旧交替不超过 1；事件单条及字段精确匹配。
