# M-06 实现 #7：定位回归测试文件保护

本轮只推进 M-06；实现轮次 7/30。上一轮审计 M-05 通过，没有待搬入测试体系的缺陷判别检查。基线为 M-05 的 1071 个用例。

## 实现与范围

- `scheduler.rs` 在定位进程启动前采集各仓库 HEAD，写入该定位会话的 `session.started.locate_heads`。需求会话的事件不新增字段。
- 定位完成后，比较起点与定位提交的 A/M 文件；以仓库、路径、定位 HEAD 和 `ls-tree -z` 的 blob/mode 保存 `bug.regression_snapshot`，复用现有事件存储，不迁移账本。
- 修复完成（包括暂停后交付结果）在进入 fix_gate 前核对 HEAD 和未提交差异。修改、弱化断言、删除、重命名、执行模式变化均转为 Bug GuardFailed → Fix/Protocol，错误指出仓库与文件。
- 空快照可以随未复现的定位结果保存，但不能放行修复门禁：修复交付明确以“保护集为空”失败。这样保留原有“复现命令成功/不可运行 → 再定位 → 等信息”的行为，不把空集称为已保护。
- 重试、追加预算、暂停使用同一持久化快照；重新定位重建。无快照的旧 Bug 与需求任务不启用守卫。
- 实际识别范围：`test/`、`tests/`、`__tests__/`；`test_`/`spec_` 前缀、test/tests/spec 文件主名、`.test.`/`.spec.`/`_test.`/`_spec.`；Rust `#[test]`、`#[cfg(test)]`、`#[tokio::test]` 与 describe/it 内联测试段；复现命令直接出现的脚本路径（脚本扩展名或 shebang，含引号、空格和工作区成员前缀）。内联段从首个测试标记到文件末尾作保守比较，发生变化则保护整个源码文件；不解析任意语言 AST 或动态生成的 shell 路径。
- 真实 Git/worktree 夹具核定了 shell 复现脚本、Rust 内联文件、JS spec 名、三类测试目录、已有测试的修改、中文/空格路径。未修改任何既有测试；新增四个指定名称的 e2e，内部使用多个正反向场景。

## 标待审之前的自审清单

1. **情形表逐行**：按 M-06 范围各行执行。
   - 单仓库定位新增/修改测试：拒绝组核对定位 blob 原文与修复后的字节；已有测试五类路径逐个实际修改并拒绝。
   - 内容/断言弱化、删除、重命名、chmod：拒绝组四个分支全部通过，断言 Fix/Protocol 和文件名，并核对内容/存在性。
   - 非测试产品修改：product-only 组通过到 await_merge；保护集六个文件逐一比对定位内容；另验证含未变化内联测试的产品源码仍可修复。
   - 空保护集：明确 Protocol 失败；带空格的直接引用脚本篡改也被拒绝。
   - 重试/追加预算/暂停：正向修复到 await_merge，篡改测试到 Fix/Protocol；快照不变且保护字节符合预期。
   - 重新定位：第二次定位更新测试，快照事件数为 2，后续产品修复通过，保留第二次定位的测试字节。
   - 工作区：分别篡改 a、b 均被对应仓库守卫拒绝；两仓只修产品则通过；两成员测试字节逐项断言。
   - 需求与旧 Bug：原有 feature 与 legacy Bug 用例保持原样，纳入全量验收。
2. **失败分支**：本里程碑点名的修改、删除、重命名、模式改变及空保护集已逐项实跑；每个失败检查均断言节点/Protocol 原因，非仅判断退出码。拒绝授权、外部服务不可达、超时不适用：本里程碑不新增授权、服务调用或超时配置。Git 读取错误沿已有 Result 转为 GuardFailed，不声称完成独立磁盘故障注入。
3. **全部迁移**：定位→复现→修复→fix_gate→复核→await_merge；修复→Protocol；崩溃→重试；门禁失败→预算用尽→extend→修复；暂停→持有结果→恢复后交付（通过/拒绝）；复现成功退出→再定位→新快照，均有本轮或原有 e2e。未增加状态机节点或转换规则。
4. **输入域边界**：空保护集、单脚本、六文件集、两仓库、已有文件修改、新增文件、中文/空格路径、可执行模式逐项通过。日期、前导零、闰日和数值最大值不适用：守卫输入是 Git 文件及提交，不新增日期或数值输入域。

## 验证过程与负向对照

- 初版全量暴露两条既有“复现未成功”的路径被空集检查过早拒绝；修正检查时机后，原有 `cargo test -p automed --test end_to_end bug_loop` 为 14/14 通过，既有测试零改动。原始记录：`.autome/output/r7-existing-bug.txt`（红）、`r7-existing-final.txt`（绿）。
- 禁用 `guard_bug_regression` 后运行 `cargo test -p automed --test end_to_end bug_regression_guard_rejects_modified_or_deleted_tests`：0 passed / 1 failed；记录 `.autome/output/r7-mutant-disabled.txt`。已还原。
- 把 `product.txt` 强制加入保护集后运行 `cargo test -p automed --test end_to_end bug_regression_guard_allows_product_only_fixes`：0 passed / 1 failed；记录 `.autome/output/r7-mutant-products.txt`。已还原。
- 最终专项 `cargo test -p automed --test end_to_end bug_regression_guard`：4 passed / 0 failed / 0 ignored，73 filtered；记录 `.autome/output/r7-special-final2.txt`。四个测试内部共 21 个真实 Git/worktree 场景（拒绝 11、产品正向/重新定位 3、重试与暂停 4、工作区 3），不把内部循环次数当成 Cargo 用例数。

## 最终验收结论

M-06 验收命令全部退出 0：

1. `cargo test -p automed --test end_to_end bug_regression_guard`：4/4，日志 `.autome/output/r7-special-final2.txt`。
2. `cargo test --workspace`：10 个测试套件合计 **1075 passed、0 failed、0 ignored**，日志 `.autome/output/r7-workspace-final2.txt`。
3. `cargo clippy --all-targets -- -D warnings`：退出 0，日志 `.autome/output/r7-clippy-final.txt`。
4. `cargo fmt --all -- --check`：退出 0，日志 `.autome/output/r7-fmt-final.txt`（无输出）。

与上一轮基线差为 **1075 - 1071 = +4**，全部是本轮指定的四个 `bug_regression_guard_*` e2e。现有用例没有删除、改写或跳过。`git diff --check` 无输出。最终源码已还原所有变异；验收后仅补任务记录。

结论：M-06 标为 **待审**，不标已完成。无环境阻塞，不启动下一个会话，交由 Autome 调度独立审计。
