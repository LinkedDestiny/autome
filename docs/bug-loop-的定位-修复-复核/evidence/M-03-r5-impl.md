# M-03 r5 实现：合并前门禁失败修复

本轮为实现 #5（5/10），按用户最后指令只修复门禁，不推进新里程碑；保留审计已关闭的里程碑状态，不新增或自行关闭里程碑。

## 失败证据与根因

用户给出的宿主尾部只能确定 Rust Test 步骤失败，不能从该尾部判断具体失败用例或失败总数。上一轮复盘与 `.autome/output/retro2-e2e-protocol-version.txt` 明确记录项目既有用例 `every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task` 在 end_to_end.rs:1734 失败：期望 `protocol/v3@`，实际 `protocol/v4@5d67a6c90db5a773dd82b3b1175adadb49fae59bfe99780bc97b49a5df8c80f8`。本轮原样重跑确认同一失败，不声称拿到了宿主完整日志或已排除其它失败。

根因：M-03 集成将任务协议升至 v4，协议单测已经同步，e2e 中手写的 v3 前缀断言遗漏更新。完整 protocol_ref 与编译内置 current().0.to_wire() 的相等断言已通过，故不是生成或冻结版本错误。

修复：只把该断言的 v3 改成 v4，保留完整引用相等、冻结文件、各角色 brief 等全部断言。该判别检查本来就在项目测试体系内，无临时检查需要搬入，无新增用例。扫描 crates 中 Rust 文件的 `protocol/v3`，发现的唯一命中就是本次修复位置；不改历史文档和测试数据。

## 命令与结果

- 红：`cargo test -p automed --test end_to_end every_round_is_handed_a_brief`，原断言原样运行，1 个用例，0 通过 / 1 失败，132 filtered，13.46 秒。日志 `.autome/output/r5-version-red.txt`。
- 绿：同一命令，1 个用例通过，132 filtered，14.32 秒。日志 `.autome/output/r5-version-green.txt`。
- `cargo test -p automed --lib protocol::`：67 个通过，659 filtered。日志 `.autome/output/r5-protocol.txt`。
- `cargo test -p automed --test prompt_rendering`：28 个通过。日志 `.autome/output/r5-prompts.txt`。
- `cargo test -p automed --test end_to_end bug_loop`：18 个通过，115 filtered。日志 `.autome/output/r5-bug-loop.txt`。
- `cargo fmt --all -- --check`、`git diff --check`：通过。

绿态测试共 114 个，0 失败、0 ignored。与上一轮对应子集基线相比，protocol:: 67→67、prompt_rendering 28→28、bug_loop 18→18，均差 0；本轮另跑 1 个已有 brief 用例，故选取的验证集合 113→114，差 +1 来自验证范围增加，不是新增测试。该 e2e 二进制总数仍为 133（1+132 / 18+115），与上一轮复盘记录一致，差 0。filtered 是未选择的用例，不能计作通过。

## 自审四项（本轮修复范围）

1. **情形表逐行**：M-03 的 2 门禁模式 × 3 Bug 步骤 × 4 输入组合由 prompt_rendering 的交叉用例覆盖，28 个测试通过；有输入与空输入的 Bug Loop e2e 子集 18 个通过。版本记录与 brief 由本轮红绿用例覆盖。仅版本期望改变，无产品行为改变。
2. **失败分支**：protocol:: 子集覆盖格式错误及旧版兼容，67 个通过；过期版本断言的失败已原样复现。拒绝授权、读盘失败、超时、外部服务不可达对本次常量断言修正不适用，因为未改对应生产路径。
3. **全部迁移**：不适用——本轮未改状态机或通道，既有 bug_loop 18 个测试作相关回归，不将它们称为全状态机验收。
4. **域边界**：版本边界 v3/v4 有红绿证据；空输入、绝对路径、空格和花括号由 prompt_rendering 交叉用例通过。数字、日期域边界不适用，本轮无此类输入解析改动。

## 范围与剩余验证

仅改 end_to_end.rs 一行及本轮证据、设计状态/证据指针、retro 一行；未改生产逻辑、协议、其它工作树或 `.autome/` 配置。原始日志按用户要求写入 `.autome/output/`。

未运行 `sh scripts/ci-local.sh` 或 workspace 全量，最终门禁由 Autome 在合并前执行。审计 #3 留下的 B-06 gate queue 时序不稳定项仍未解决，不能据本轮子集通过宣称全量通过。上一轮审计结论为通过，未退回产品缺陷；本轮处理的是随后复盘发现的既有回归测试失败。
