# M-02 · 实现 #2

范围：仅 M-02（A-1、A-2 展示与词表、验-1、验-2、约束 4）。宿主起点 `469094dbf995aca268863fedcf9c0a703dd013e8`。上一轮审计通过 M-01，没有退回缺陷或需要逐字搬入的判别检查。

## 实现与修复说明

- 自动与手动复盘共用的 launch_session 指标分支调用 retro_task_metrics：现算成功直接交给提示词；失败记录 warn 后读存量，存量读取再失败则 warn 并返回 None。不改变最终 write_task_metrics 的时机。
- 指标表在人工验收与 tokens 之间增加每轮门禁、合并前门禁、门禁耗时；次数或失败数任一缺失，整行显示「未记录」。词表直接使用 M-01 扩展后的 METRIC_NAMES。
- 新增 P1–P3、E1–E2，以及一个真实 SQLite 损坏数据的失败分支单测。没有改变协议、schema、M-03 或真实账本。
- E1 初次执行时，测试把夹具实现预算硬编码为 15，实际内核对单里程碑计算预算为 5，导致断言失败；改为读取该夹具任务的 budget_n。未修改产品预算规则，本任务状态仍为 2/15。

## 验收命令与用例总数

`cargo test -p automed --test prompt_rendering && cargo test -p automed --test end_to_end retro`

- prompt_rendering：52 个全部通过；上一轮设计基线 49，增加 P1–P3 共 3 个。
- end_to_end retro：9 个全部通过；上一轮基线 7，增加 E1–E2 共 2 个。
- 验收合计 61 个（基线 56，+5），0 失败、0 忽略。故障注入还原后重新执行，两段仍全过。原始输出 `.autome/output/m02-prompt.log`、`m02-retro.log`。
- 直接相关子集 `cargo test -p automed --lib retro_metrics_read_failure`：1 个通过，原基线 0，新增失败退路用例；输出 `.autome/output/m02-fallback.log`。与验收合计 62 个独立用例，基线 56，+6。
- 按本轮用户限制，没有额外主动运行无过滤 lib 或完整 end_to_end；指定快检选中的测试另记。没有运行 `sh scripts/ci-local.sh`。

## 标待审之前的自审清单

### 1. 情形表逐行

- A-Q1 自动复盘：E1 两次启动分别看到每轮 2/3 次（失败均 1），合并前 0/1 次（失败 0/1），证明每次重算。
- A-Q1 手动复盘：E2 暂停任务无历史指标，启动后得到指标表，仍停在 paused。现有失败任务手动复盘、运行中拒绝手动复盘、恢复原节点的用例也通过。
- A-Q2 在途不落账本：E1 两次复盘暂停点的 tasks_with_metrics 均不含任务，task.metrics 事件为 0；E2 复盘后 metrics 仍空且事件为 0。
- A-Q2 最终计量：E1 合并清理后为每轮 3 次/失败 1、合并前 2 次/失败 1，逐 node 与真实 gate.result 扫描结果比较一致，gate_seconds 为 Some。
- A-Q5 三行格式与位置：P1 逐行精确匹配 3/1、2/1、812 秒，并断言位于人工验收与 tokens 之间。
- A-Q5 缺失：P2 分别检查次数缺失、失败数缺失、两者均缺失及耗时缺失，均为未记录而非 0 次；原有整个 metrics=None 的提示断言仍过。
- A-Q3 词表：P3 逐个检查五个新名字全部出现在复盘提示词。

### 2. 失败分支

- 现算读账本失败：SQLite gate.result payload 改成非法 JSON，先断言 measure_task 报错，再断言返回预先保存的指标。
- 存量为空：上述现算失败条件下将 tasks.metrics 置 NULL，结果 None。
- 存量读取也失败：将 tasks.metrics 改为非法 JSON，结果 None，未发布 task.metrics 事件。以上均在 retro_metrics_read_failure 单测中实跑通过。
- 拒绝授权、外部服务不可达、超时不适用：本改动是本地同步指标读取与渲染，不增加授权、网络调用或等待；门禁超时计数已属 M-01。

### 3. 全部迁移

- E1 实测会话序列：1 整理、2 设计、3 评审、4 裁决；5 实现 → 快检 1 失败 → 6 实现 → 快检 2 通过 → 7 审计 → 8 复盘 → 合并前门禁 1 失败 → 9 实现 → 快检 3 通过 → 10 审计 → 11 复盘 → 合并前门禁 2 通过 → await_merge → task.merge → 清理 → done。
- E2：实现运行 → task.pause → 会话收尾并暂停 → task.retro → 复盘完成 → 仍 paused。现有 7 个 retro 用例覆盖复盘恢复、运行中拒绝以及范围违规的回退路径，均通过。
- 数据迁移：无指标 → 现算值只进提示词 → 再次现算包含新增事件 → 最终持久化。失败迁移覆盖现算失败 → 存量 / None、存量失败 → None。本轮不新增状态机状态。

### 4. 输入域边界

- 无门禁事件：E2 显示实测 0 次（失败 0）；首次自动复盘的合并前门禁也是 0。
- 缺失整个指标、缺失一个字段、缺失全部新字段：原用例与 P2 通过，None 与 Some(0) 不混淆。
- 重复启动与累计值变化：E1 两次复盘均独立检查；存量损坏与空存量由失败分支单测覆盖。
- 日期文本、前导零、闰日、跨年、空串及可配置最大输入不适用：本轮接入接收既有强类型计量结果，没有增加文本解析或输入域；事件时间口径与数值聚合未改。

## 负向对照

均以 `cargo test -p automed --test end_to_end retro_gate_metrics` 执行 2 个新用例，注入后还原生产源码。

1. 复盘分支改回 `ctx.store.task_metrics(&task.id)?`：2 例全部失败（退出 101），无法得到当时指标。输出 `.autome/output/m02-negative-stored.log`。
2. 复盘分支先调用 write_task_metrics 再读指标：2 例全部失败（退出 101）；E1 在途任务出现在 tasks_with_metrics，E2 暂停任务 metrics 非空。输出 `.autome/output/m02-negative-write.log`。
3. 还原后 61 个验收用例全绿。预期红不计入最终失败数。

## 指定快检与交接

命令：`sh scripts/ci-quick.sh --since 469094dbf995aca268863fedcf9c0a703dd013e8`。

退出 0，8 步执行、0 沿用、0 失败，总计 87 秒；相关测试阶段 69 秒，准入预算 120 秒。Format、Clippy、Web lint/typecheck 均通过。输出 `.autome/output/m02-quick.log`。

快检选中 prompt_rendering 52 个与 automed lib 1054 个，共 1106 个全过。按同一选择范围，上一轮基线为 49 + 1053 = 1102，增加 4 个（P1–P3 与读取失败单测）。上一轮快检选的是 domain/automed/CLI 共 1609 个，本轮少 503 个是选择范围变化：不再选 domain 489 与 CLI 67，新增选 prompt 52，automed 增加 1，即 -489-67+52+1=-503，不能据此声称全项目测试数量减少。

快检明确未跑完整 end_to_end（改动不全在既有测试函数体内，留给合并前门禁）；本轮独立跑的 retro 9 个全过。快检与本轮验收去重后覆盖 1115 个用例，同范围上一轮基线 1109，增加 6 个，0 失败/忽略。基线来自既有证据与设计数量，没有重跑旧提交。

`cargo fmt --all -- --check` 与 `git diff --check` 通过。四项自审完成后，M-02 标为待审，implementation-round 为 2/15，保留 M-01 已完成、M-03 开放；retro 仅追加一行。等待独立审计，不启动下一个会话。
