# M-01 实现 #1 证据

范围：只实现设计 §3、§4、§7 的纯函数与配置；调度器接入、作业 I/O、读者与超时文案属于后续里程碑。本轮没有上一轮审计，故无审计检查需要搬迁。开工 HEAD 为 `04c3071`。

## 改动

- 新增 domain `gate_queue::{Waiter, Admission, admission, position, queued_after_event}`；按事件 seq 算名次，严格 FIFO，不做 I/O。输入为每任务一个有效排队项，seq 为账本唯一序号，limit 来自已解析配置。
- 全局 `GlobalGateConfig` 缺省 1，项目可选 `gate_concurrency`，resolve 取项目与全局较小值；旧序列化数据缺字段时仍取 1。
- 全局/项目 TOML 都拒绝非 1–10 的整数，错误为 `[gate] concurrency 必须是 1 到 10 的整数`。仅写 concurrency 时无门禁命令；显式非字符串 command 仍拒绝。
- 保持原有保存实现：不改 gate 表，手写配置不丢。原有测试无删除、无改名、无修改。

## 命令与结果

原始输出均在 `.autome/output/`，未转存进任务目录。

- 开工 `cargo test --workspace -- --list`：1201 个，清单 `m01-baseline.txt`。domain 385 = 384 单测 + 1 原有忽略的集成测试；automed lib 648，CLI 39（25 + 14），end_to_end 106，prompt_rendering 16，另 7 集成测试。与设计基线一致；没有上一实现轮。
- `cargo test -p autome-domain && cargo test -p automed --lib`：退出 0。domain 共 394（393 通过、0 失败、1 原有忽略），相对基线 +9（FIFO/事件 6，配置解析后取值 3）；automed lib 共 655，全部通过，相对基线 +7（配置读写）。输出 `m01-domain.txt`、`m01-automed.txt`。
- 完成后 `cargo test --workspace -- --list`：1217，总数 +16；逐名比较新增 16、减少 0。输出 `m01-final-list.txt`、`m01-list-delta.txt`。此命令只列出用例，不声称全工作区实跑。
- `cargo clippy -p autome-domain -p automed --lib -- -D warnings`：退出 0，`m01-clippy.txt`。
- `cargo fmt --all -- --check`、`git diff --check`：通过。

## 标待审之前的自审清单

1. **情形表逐行**：M-01 契约 16 项全部落实为项目测试。队首有空位 Go；非队首即使有空位仍 Wait；新来者到队尾；满额及超额均 Wait；小名额队首挡住大名额后排；乱序输入按 seq 给出从 1 起的名次，未知任务无名次。配置缺省 1、项目覆盖、全局封顶、全局读取/非法值、项目读取/非法值、无 command、全局保存、项目保存逐项通过。§5 的作业接入表不适用本里程碑，归 M-02。
2. **失败分支**：全局与项目的 gate 非表均断言中文报错；0、-1、1.5、字符串 '2'、11、布尔、数组、空串、超 u32 值均拒绝；项目带/不带 command 两路均检查；显式数字 command 拒绝；原有缺 command 用例不变并通过。授权拒绝、读盘故障、超时、外部不可达不适用新增纯函数/解析逻辑，没有新增这些 I/O 分支；保存路径沿用现有实现。
3. **全部迁移**：`gate_queue_event_transitions` 对 queued=false/true 各检查：gate.queued → true；gate.started、setup.started、session.started、session.deferred、task.updated、gate.queue_dropped → false；gate.result、setup.result、未知/空事件保持原值。所有状态 × 事件组合均通过。没有新增 TaskState 迁移。
4. **域边界**：空队列、单元素、未入队者、无序 seq、i64::MAX seq、held=usize::MAX、满额/超额、limit=1/10、空配置、空 gate 表、前导零 01（TOML 拒绝）、空串与上下界外值均执行通过。日期/闰日/跨年不适用，此接口无日期输入。

## 负向对照

- 把 FIFO 条件换成 `held + position - 1 < limit`，在 output 下的独立源码副本运行同一个项目测试 `gate_queue_smaller_head_limit_blocks_larger_followers`：1 失败，证明能抓越位；未改正式源码。输出 `m01-fifo-negative.txt`。
- 临时移除 resolve 的 `.min(global.gate.concurrency)`，运行 `cargo test -p autome-domain gate_concurrency_cannot_exceed_global`：1 失败。还原后 `cargo test -p autome-domain gate_concurrency_`：3 通过。输出 `m01-cap-negative.txt`、`m01-cap-green.txt`。
- 临时让 save_global 清空已有文件，运行 `gate_concurrency_save_global_preserves_handwritten_value`：断言失败；还原后配置 7 用例全通过。首次注入使用 `?` 因错误类型不兼容未编译，该次不算判别证据；改用 unwrap 后才获得真实断言失败。输出 `m01-save-negative-retry.txt`、`m01-save-green-final.txt`。

## 限制与交接

全工作区实际运行、e2e、桌面验收按 M-05/M-06 执行，本轮没有代称通过。原有忽略集成测试保留，本轮未强行启用。M-01 可交审计独立复验；M-02 尚未实现。
