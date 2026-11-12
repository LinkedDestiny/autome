# M-03 实现轮 #3

范围：仅推进 M-03（读者与 CLI）；上一轮 M-02 审计通过，无退回缺陷及必须逐字搬入的回归检查。

## 实现

- task.get 增加 gate_queue_position，复用事件折叠与 domain 的 position；原 queue_position 与 gate 不变。
- dashboard.get 一次读取有效队列，为 waiting / running 每项提供同名字段；跨项目按事件顺序计算实时名次。
- CLI status 列表、status <id> 详情、run 的 gate.queued 事件分别显示当前排队和入队快照；不新增状态、方法或白名单项。
- 新增 dispatch 3 个、render 3 个用例，未修改任何旧用例。
- 首次编译发现 waiters 的 Box<dyn Error> 不能直接用 ? 转为 DispatchError，已按现有 internal 映射处理，未扩大错误类型接口。

## 验收与基线

- 验收命令：`cargo test -p automed --lib && cargo test -p autome-cli`，退出 0。automed 679/679；CLI 42/42（28 单元 + 14 集成）；失败、忽略均 0。
- 上一轮 automed --lib 基线 676，CLI 基线 39；本轮合计 721，对比 715 增加 6，全部来自新增 dispatch 3 个与 render 3 个。旧测试一行未改，未删减旧用例。
- 原始输出：`.autome/output/m03-automed.txt`（83.97 秒）、`.autome/output/m03-cli.txt`。
- 新增过滤复跑：`cargo test -p automed --lib dispatch::tests::gate_queue_` 3/3；`cargo test -p autome-cli gate_queue_` 3/3。输出为 `m03-reader-green.txt` / `m03-cli-green.txt`（均在 .autome/output）。
- 负向对照由 `.autome/output/m03-mutations.rb` 执行并自动还原：把 task.get 新字段错写为 queue_position，实时名次测试退出 101（Null != 1）；把 gate_queue_line 的 position-1 改为 position，人数测试退出 101（1 != 0）。还原后的上述过滤复跑均绿。原始输出为 `m03-negative-field.txt`、`m03-negative-off-by-one.txt`。


- `cargo clippy -p automed -p autome-cli --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check` 均退出 0；原始输出在 .autome/output/m03-clippy.txt、m03-fmt-check.txt、m03-diff-check.txt。

## 标待审前自审清单

1. **情形表逐行**
   - task.get 排队 / 不排队：分别返回 1 起实时名次 / 显式 null；并行槽位 queue_position 保持 null，状态保持 active/gate。
   - dashboard running / waiting：跨项目两个排队项分别返回 1、2；启动队首后分别为 null、1；waiting 的非排队项显式 null。
   - CLI status / status <id>：核对 main.rs 两处调用 gate_queue_line；run 由 event_line 单测覆盖节点与“入队时”措辞。
   - state_line：五种门禁节点保持原文；gate.queue_dropped / bug.fix_round_requested 继续忽略。
2. **失败分支**
   - 不适用：M-03 是已有账本数据的只读展示，未新增授权、超时或外部服务分支；作业目录读盘失败属 M-02，本轮不读取作业目录。账本读取错误按现有 RPC internal 返回，不伪装空队列。
3. **全部迁移**
   - 用例逐项执行：未排队→gate.queued→有名次；gate.result 不改变名次；gate.started、setup.started、session.started、session.deferred、task.updated、gate.queue_dropped 分别使名次变 null；每次再入队恢复为 1。
   - 队首启动→后继名次 2→1，任务创建顺序与入队顺序相反仍按事件排序；dashboard 同步验证。
4. **输入域边界**
   - 无有效排队项为 null；单元素名次 1 显示前面 0 个；名次 2 / 3 分别显示 1 / 2；u64::MAX 不溢出；额外容错 position=0 饱和为 0。
   - 不适用：日期、前导零、空串不是本里程碑的输入域（名次为 JSON 整数）。

## 限制

真实终端人工验收 H-01 未执行；全 workspace、桌面端与 e2e 的最终联合验收留在 M-05/M-06。本轮不启动下一会话、不推送远端。

结论：四项自审已执行，M-03 验收通过，报待审；不代替独立审计。
