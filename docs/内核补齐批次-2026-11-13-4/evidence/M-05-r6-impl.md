# M-05 · 实现 #6（快检修复轮）

本轮只修复宿主在 e27b6f5ef3 上发现的 Clippy `items_after_test_module`，不推进新里程碑，M-05 保持待审。上一审计结论为 M-04 通过，无审计退回用例需要搬入。

## 修复与红绿证据

将 `crates/automed/src/handoff.rs` 的 `test_fix_section()` 原样移到 `#[cfg(test)] mod tests` 前，函数内容、签名、测试和运行行为均不变；不加 lint allow。现有项目快检及 CI 的 `cargo clippy --all-targets -- -D warnings` 已是该缺陷的回归检查，无需新增镜像结构测试。

- 红：原样运行 `cargo clippy --all-targets -- -D warnings`，编译失败，诊断明确指向 handoff.rs 的测试模块之后存在函数。日志 `.autome/output/m05-r6-clippy-red.txt`。包装命令最后执行 tail，包装退出码 0 不代表 Clippy 成功，以日志中的编译错误为红证据。
- 绿：运行用户指定完整快检 `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web`，退出 0。日志 `.autome/output/m05-r6-quick-green.txt`。Clippy 全 targets 同时检查其它同类位置，无剩余报错。
- `cargo test -p automed --lib handoff::tests`：2 个通过，0 失败、0 跳过；退出 0。日志 `.autome/output/m05-r6-handoff.txt`。
- `cargo test -p automed --test end_to_end test_fix_e2e_review_can_request_repair_and_stale_declaration_is_rejected`：1 个通过，0 失败、0 跳过；退出 0。日志 `.autome/output/m05-r6-prompt.txt`。

本轮实际测试总数 **3**，对比上一轮 75 个不同用例为 **-72**，原因是本轮只移动函数，范围收缩为 handoff 两个单测和直接涉及定位提示的一个 e2e；不是删除测试。同一筛选范围前后均为 3，差 **0**。快检为静态检查，不计为测试用例。本轮未重跑 M-05 全部验收，不将上一轮结果冒充本轮实跑。

## 自审清单

1. **情形表逐行**：本轮唯一改动为函数顺序。核对 diff，函数逐字不变；handoff 两个测试覆盖空补丁、普通/全部关闭里程碑、Bug 文案、待办顺序与过滤；e2e 验证复核退回修测试及提示。全部通过。M-05 其它情形未改，本轮不重新报验。
2. **设计点名的失败分支**：直接相关的定位轮残留声明拒绝路径由上述 e2e 实跑通过；Clippy 结构失败红绿均已执行。读盘失败、超时、外部服务不可达等不适用本轮函数搬移，不新增 I/O 或异常路径。
3. **全部状态迁移**：本轮不改变状态机，完整迁移复验不适用；直接相关 e2e 实跑复核声明后退回定位及残留声明拒绝，结果通过。其余迁移保留上轮证据，本轮未重跑。
4. **输入域边界**：移动的函数无参数、返回固定文本，输入域不适用；既有 handoff 单测仍检查空集合和不匹配任务，无新增边界。

## 交接

M-05 保持待审，implementation-round 为 6/80；下一步由宿主重跑快检并调度独立审计。快检最终结论以宿主提交后复跑为准。未运行全量门禁 `sh scripts/ci-local.sh`，未启动下一会话。
