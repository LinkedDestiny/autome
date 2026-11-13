# 实现 #7：rebase 集成修复证据（关联 M-03 / M-04 / M-05）

结论：rebase 完成，相关验收通过，交独立集成复验。本轮开始时五个里程碑均已完成，没有开放里程碑；按用户明确的 rebase 指令处理集成，不新造里程碑、不重新关闭或改成待审，全部审计证据指针原样保留。状态轮次为 7/25。上一轮审计通过，无实现缺陷退回，也无需要逐字搬入的红绿回归检查。

## rebase 与修复范围

- 轮前 HEAD：`23bd7bb3654f65fba5fd8e695e3a677716560673`；默认分支 main：`be6ffb20988be75782b2b6ca49fde0a48a2bbbb0`。
- 执行 `git rebase main`；逐个解决冲突，`git add` 后以 `GIT_EDITOR=true git rebase --continue` 继续两次，最终成功，rebase 后 HEAD 为 `a6d4a91d`。没有 merge、切分支、推送或启动下一会话。
- `crates/automed/tests/prompt_rendering.rs`：保留上游格式化约束和 timeout 金样测试，同时保留本任务四个集成测试；只拼接双方新增内容，没有削弱断言、修改测试前提或更新金样。
- `protocol/CHANGELOG.md`：保留上游 v7/F-2，本任务 J-1/J-2/J-3 归入独立 v8；v1–v8 标题连续且唯一。依设计 R4 同步当前设计与 M-05 标题计数，将协议版本单测的精确期望从 v7 改为 v8，两个哈希断言不动。
- 后续重放还触发 `crates/automed/tests/end_to_end.rs` 追加区冲突：完整保留 main 文件，追加本任务 integration helper 与四个用例。相对 main 无删除行，故上游格式化和 timeout 测试的准备条件与断言均原样保留。
- `git range-diff "$(git merge-base 23bd7bb3 main)..23bd7bb3" main..HEAD` 保存于 `.autome/output/r7-range-diff.txt`。34 个提交均保留；变化为 scheduler 上游上下文、两处测试追加位置、协议版本头。普通重放不作额外产品改动。
- 原始 rebase 输出：`.autome/output/r7-rebase.txt`、`r7-rebase-continue.txt`、`r7-rebase-continue2.txt`；测试差异：`r7-test-diff.txt`。本轮后续提交只同步版本断言与任务记录。

## 验收命令与结果

以下均退出 0；命令分步执行，结果与契约要求相同。

| 命令 | 用例 / 结果 | 原始输出（均在 `.autome/output/`） |
|---|---|---|
| `cargo test -p automed --test prompt_rendering` | 40 通过，0 失败、0 忽略 | `r7-prompt-initial.txt`、`r7-m05-final.txt` |
| `cargo test -p automed --lib integration_section` | 6 通过，0 失败、0 忽略 | `r7-section.txt` |
| `cargo run -p automed -- protocol eval protocol/` | 8 项通过，0 失败，1 项既有约束溯源警告（41 行） | `r7-eval.txt`、`r7-m05-final.txt` |
| `git diff --quiet --diff-filter=MD main -- crates/automed/tests/golden/` | 既有金样未修改或删除 | 退出 0 |
| `cargo test -p automed --test end_to_end integration_e2e` | 4 通过，0 失败、0 忽略 | `r7-e2e.txt` |
| `cargo test -p automed --test end_to_end before_merge` | 11 通过，0 失败、0 忽略 | `r7-e2e.txt` |
| `cargo test -p automed --test end_to_end a_gate_failing_after_the_automatic_rebase` | 1 通过 | `r7-e2e.txt` |
| `cargo test -p automed --test end_to_end a_failing_gate_sends_the_work_back` | 1 通过 | `r7-e2e.txt` |
| `! git diff -U0 main -- crates/automed/tests/end_to_end.rs \| grep -q '^-[^-]'` | e2e 相对 main 只增不改 | 退出 0 |
| `cargo test -p automed --lib the_version_in_force_is_the_newest_changelog_heading_over_the_seed` | 1 通过，版本 v8 与两个哈希断言成立 | `r7-version.txt` |
| `cargo test -p automed --test end_to_end regression_format_only` | 5 通过 | `r7-upstream-e2e.txt` |
| `cargo test -p automed --test end_to_end timeout_retry` | 2 通过 | `r7-upstream-e2e.txt` |

M-05 当前完整验收链亦原样运行：

```sh
cargo run -p automed -- protocol eval protocol/ && cargo test -p automed --test prompt_rendering && test "$(grep -c '^## protocol/v' protocol/CHANGELOG.md)" = 8 && grep -q '^## 集成复验$' protocol/loop-protocol.md && grep -q '集成复验' docs/development/technical-design.md
```

退出 0。版本计数由 7 改为 8 是保留上游已发布 v7 后顺延版本，未放宽校验。

**用例总数与基线：** 本轮相关 Rust 子集去重共 **71 个**（40+6+17+1+5+2），全部通过，失败/忽略均 0，重复运行 prompt 不重复计数。与上一轮 M-05 同口径比较：prompt 40 对 35，**+5**，来源是 main 的两个格式化 prompt 回归与三个 timeout prompt 金样。M-03 的 integration_section 6、M-04 的 17 均与此前基线一致，差 0。另跑版本单测 1 与解决追加冲突所涉及的上游 e2e 7，属于本轮补充检查，不冒充上一轮同命令数量增幅。本轮没有新建、删除或跳过测试用例。

## 标待审之前的自审清单（本轮为集成修复，仍逐项执行）

1. **情形表逐行**：M-03 默认/集成、普通/合并前 prompt 与简报均由 40 个金样和 6 个 integration_section 检查覆盖；M-04 四行依次为 rebase→独立审计→合并卡点、门禁修复→审计退回、缺证据→停止、纯文档→原流程，四个 integration_e2e 均通过；M-05 独立二级节、技术设计、静态元数据、v1–v8 连续唯一均通过。冲突两侧测试均保留，相对 main 的既有金样零改动、e2e 只新增。
2. **失败分支**：无审计证据被阻断的集成 e2e、门禁失败后退回的 e2e、模板标记不配对的 integration_section 均通过；保留下来的 timeout 两个 e2e、格式化保护五个 e2e 亦通过。拒绝授权、读盘失败、外部服务不可达不适用本轮：未改对应实现，不声称本轮重新跑过 M-01 全集。
3. **全部迁移**：四个 integration_e2e 覆盖通过→retro/rebase/await_merge、退回→开放/reopen+1/implement、无证据→failed、纯文档→无集成事件的原路径；before_merge 与两个门禁回归共 13 个亦通过。上游 timeout 暂停/恢复和耗尽路径由保留的两个 e2e 验证。本轮无新增状态迁移。
4. **域边界**：版本边界检查 v1–v8 连续唯一、current() 精确返回 v8 且模板哈希一致；集成标记缺失/存在与不配对、无集成段时原文不变、SHA 七字符由 integration_section 验证。逐个扫描 `main..HEAD` 的 34 个提交：`git grep -nE '^(<<<<<<<|>>>>>>>) ' <提交> -- .` 均退出 1，无冲突标记，扫描脚本退出 0，输出 `r7-conflict-scan.txt` 为空。日期、数值上限等无新增输入域，不适用。

附加核对：`git merge-base --is-ancestor main HEAD`、`git diff --check` 均退出 0；对两个冲突测试文件运行 `rustfmt --edition 2024 --check crates/automed/tests/prompt_rendering.rs crates/automed/tests/end_to_end.rs` 退出 0。脚本核对轮前/轮后五个里程碑状态和全部「最新证据」行相同，状态块七字段、7/25、current-milestone 无符合协议。未运行项目全量门禁、宿主快检、付费行为 eval；宿主仍需按用户指定流程运行快检和合并前门禁。
