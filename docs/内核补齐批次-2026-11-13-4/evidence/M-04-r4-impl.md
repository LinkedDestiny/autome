# M-04 · 实现 #4 · 待审

## 范围与修复说明

落实设计 A-R6 / A-V7：只新增一个 protocol/v9 标题，A-1 behavioral 指向新 eval，A-2–A-4 clarify 分别说明 Bug 协议、复核、定位边界。Bug 协议的新可选头部键及缺陷小节在 kernel-contract 区之外；fix 加 `<!-- regression-test-defect -->`；fix-review 明确声明与修测试复核；locate 按注入的修正回归测试节执行。未修改任务冻结的 protocol/v8。

新增夹具：产品错误接受空串，而回归脚本错误地用空格模拟空串；契约明确空格合法。grader 同时要求修好产品、测试逐字节不动、完整声明，拒绝迁就错误测试。新增 5 个 eval 文件并加入编译 seed（128 → 133），新增标记测试，版本断言改 v9；核对并接受 11 个 Bug 金样差异。内核守卫、成对检查调度和运行时边界属于 M-05，本轮未推进。

上一轮审计 M-03 通过，无退回缺陷，无需搬入审计缺陷检查。

## 命令、结果与用例数

验收命令原样执行（各段原始输出分别落盘）：

`cargo run -p automed -- protocol eval protocol/ && cargo test -p automed --lib protocol:: && cargo test -p automed --test prompt_rendering && test "$(grep -c '^## protocol/v' protocol/CHANGELOG.md)" = 9`

- 静态门退出 0：8 项通过、0 不通过、1 条未覆盖强制条文警告；6 个契约区不变，正文 24477 / 24576 字节，22 个 eval 可读，28 个 CHANGELOG 条目。日志 `.autome/output/m04-final-eval.txt`。
- Rust 测试总数 **111 = 71 + 40**，全过、0 忽略。与本轮改动前同范围基线 **110 = 70 + 40** 相比 **+1**，唯一新增为 `test_fix_marker_is_in_the_v9_fix_prompt`，金样测试数量不变。70 的实测见 `m04-baseline-protocol.txt`；40 与上一轮设计契约一致，本轮旧金样运行也是 40（30 过 / 10 败）。上一轮审计的 477 是域库全量，不属于本轮命令范围，不能把范围变化当删用例。
- 最终测试日志 `.autome/output/m04-final-protocol.txt`、`m04-final-golden.txt`。版本标题计数 9，整条命令退出 0。
- 初次 protocol 运行 63 过 / 8 败，原因是新 eval 未加入显式编译 seed；补齐后 71 全过。金样更新模式第一次运行 38 过 / 2 败，其中一个是并行更新时读旧金样，另一个是更新器为 audit 加了本轮无关的集成段；恢复 audit 原金样后普通模式 40 全过。没有修改测试断言来放行。
- `rustfmt --edition 2024 --check crates/automed/src/protocol/mod.rs crates/automed/src/protocol/evals.rs` 与 `git diff --check` 均退出 0。未跑项目全量门禁或宿主快检组合。

## 标待审之前的自审清单

1. **情形表逐行**：本里程碑以 A 协议落法逐项检查。
   - 产品修好但夹具错误：eval 夹具演练中，产品空串 / 空格 / good 的契约检查通过，原测试仍红；grader 要求声明四项，不允许改测试。
   - 断言与契约不符 / 格式门禁之外的测试自身问题：Bug 协议与 fix 明确列出两类；纯格式问题沿用原例外，不得用声明掩盖未修产品；逐行 diff 已核对。
   - fix 声明：所有 each_round、before_merge、quick、附件金样包含标记、待复核和四项；71 + 40 测试验证。
   - fix-review 声明及后续复核：四类金样均含打回、四项、不得改测试、改动与说明一致和不得弱化断言；diff 核对及金样通过。
   - locate 修测试：普通 / 卡片 / 附件三个金样含按注入节边界执行；协议要求删缺陷键、已定位、待复核。
   - A-1–A-4 与版本：静态门、版本断言、标题计数通过；新 eval 在磁盘和编译 seed 中均存在。
2. **失败分支**：执行设计点名的三个负向对照，全部红，恢复后完整验收绿。
   - 把 `回归测试: 有缺陷` 插入候选协议的 `bug-status-block`，`cargo run -p automed -- protocol eval .autome/output/m04-candidate` 退出 1，点名契约区被改。日志 `m04-negative-contract.txt`。
   - 同一候选恢复契约后移除 A-1 eval 目录，同命令退出 1，点名目录不存在。日志 `m04-negative-eval.txt`。
   - 暂时删除源码 fix 标记，`cargo test -p automed --lib test_fix_marker_is_in_the_v9_fix_prompt` 退出 101，1 个测试失败；恢复后最终 71 个通过。日志 `m04-negative-marker.txt`。
   - 执行脚本备份 `.autome/output/m04-negative.sh`，总结 `m04-negative-summary.txt`；候选修改只在输出目录，源码标记已还原。拒绝授权、读盘失败、超时、外部服务不可达不适用：本里程碑不新增对应运行时路径。
3. **全部迁移**：运行时状态机不适用（M-03 已实现，M-05 接线）。本轮逐条核对协议文字覆盖 fix 声明→定位、fix-review 声明→定位、定位清键→成对检查，以及接受→门禁/复核、HEAD 仍红→修复、基线不红→未复现规则、超过两次→人工处理；均与设计一致。模板通道 each_round / before_merge / quick / 附件的最终金样全过。没有将文本检查冒充调度验收。
4. **输入域边界**：声明键省略 / 唯一合法值有缺陷、四个非空项、次数上限 2 均在协议写明；解析边界实现属于 M-03，本轮无解析改动。eval 用空串、单空格、普通非空 good 演练，三种产品返回均符合契约。`sh /tmp/m04-fixture.sh`（副本 `.autome/output/m04-fixture.sh`）进一步确认原产品+错误测试红、修好产品+错误测试红、修好产品+正确测试绿、基线产品+正确测试红；日志 `m04-fixture.txt`。这些是夹具判别检查，不计入 111 个项目测试，也不等于付费模型行为 eval。日期、前导零、最大整数等不适用：此里程碑没有新增此类输入域。

## 未执行项

H-01 真实模型 `--behaviour` 改前红/改后绿尚未执行，按设计由人工验收承担；静态 eval 与夹具演练不替代它。宿主合并前全量门禁与会话后快检均未运行。
