# M-07 实现轮 #7

## 改动与审计输入

- 按设计 §6.5 原句更新 `protocol/bug-protocol.md`：`before_merge` 下修复门禁只跑复现命令，完整项目门禁留到合并前。
- `protocol/CHANGELOG.md` 的 v3 节追加 G-6：`clarify`、`bug-protocol.md`、`eval: null`、`direction: flat`，证据与设计 §7 一致。
- `crates/automed/src/protocol/phrases.rs` 的 REQUIRED 增加该句，why 引用 B-02 与目标 3。
- 上一轮审计 M-06 通过，没有退回缺陷或须逐字搬入的检查。本轮按设计不新增用例，不改模板、金样、loop/session 协议。

## 验收命令与结果

设计 M-07 验收命令原样执行：

```sh
grep -qF '修复门禁只跑复现命令，项目门禁留到合并前门禁' protocol/bug-protocol.md && grep -qF 'id: G-6' protocol/CHANGELOG.md && cargo test -p automed --lib protocol:: && cargo run -q -p automed -- protocol eval protocol/ && cargo test -p automed --test prompt_rendering && git diff --quiet $(git merge-base HEAD main) -- protocol/loop-protocol.md protocol/session-protocol.md
```

退出 0。原始输出：`.autome/output/m07-acceptance.txt`。

- protocol 单测总数 67，全部通过，相对上一轮 M-06 的 67 差 0。
- prompt_rendering 总数 20，全部通过，相对上一轮 20 差 0。两组共 87 个用例，基线 87，差 0；没有新增、删除或跳过用例。
- protocol eval：19 个用例可读且带阳性对照，相对基线 19 差 0；必含/禁含短语 89 条，相对 88 增加 1（本轮 Bug 协议短语）；CHANGELOG 11 条，相对 10 增加 1（G-6）。8 项通过、0 项不通过，1 项既有历史条文缺理由警告。
- `git diff --quiet HEAD -- protocol/prompts crates/automed/tests/golden` 退出 0；loop/session 协议相对 merge-base 检查退出 0。
- `cargo fmt --all -- --check`、`git diff --check` 均退出 0；格式输出在 `.autome/output/m07-fmt.txt`。未运行完整宿主门禁和付费真实模型 eval，不属于本里程碑验收命令。

## 标待审之前的自审清单

1. **情形表逐行**：设计 §6.5 原句逐字一致；§7 G-6 行六项字段及证据一致；REQUIRED 文件名、短语和 why 符合要求；模板/金样与 loop/session 协议未改。上述验收及 diff 检查均通过。默认模式说明保留，新增条件明确限定 `before_merge`。
2. **失败分支**：逐项执行设计 M-07 三项负向对照（见下）；拒绝授权、读盘失败、超时、空输入、外部服务不可达不适用，本轮只补静态协议约束，没有新增这些运行分支。
3. **全部迁移**：不适用，本轮不改调度状态机或通道；只澄清已实现的 FixGate 与 MergeGate 分工。现有 20 个渲染用例全通过，模板字节不变。
4. **输入域边界**：新 why 长度约束已用不足 20 字节的 `short` 检查拒绝，恢复正常理由后通过；G-6 的 `eval: null` 在 clarify 合法、behavioral 非法，两种均实跑。数字、日期、空串等运行输入域不适用，本轮没有新增解析输入域。

## 负向对照：红 → 还原/补全 → 绿

- 先增加 REQUIRED，保留旧 Bug 协议原句：短语 grep 退出 1；`cargo test -p automed --lib the_seed_passes_the_static_and_parse_layers` 退出 101（1 个失败），明确报 bug-protocol.md 缺短语；`cargo run -q -p automed -- protocol eval protocol/` 退出 1，报告同一原因。随后按设计补全原句；最终验收含该测试及 eval，均绿。输出：`.autome/output/m07-missing-phrase-test.txt`、`m07-missing-phrase-eval.txt`。
- 暂把 G-6 改为 `kind: behavioral`、保持 `eval: null`：同一 eval 命令退出 1，明确报「G-6：behavioral / retire 改动必须指明 eval 用例」。恢复 clarify 后最终验收退出 0。输出：`.autome/output/m07-behavioral-null.txt`。
- 暂把新 why 改为 `short`：`cargo test -p automed --lib every_requirement_carries_a_reason` 退出 101（1 个失败），明确报新短语没有可用理由。恢复完整理由后最终 67 个 protocol 测试包含该检查并通过。输出：`.autome/output/m07-short-why.txt`。

全部临时错误已还原。结论：M-07 验收通过，提交独立审计；不关闭里程碑。
