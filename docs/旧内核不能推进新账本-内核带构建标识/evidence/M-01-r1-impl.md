# M-01 · 实现 #1

## 范围与修复说明

只推进 M-01。新增 build.rs、共享 build_support、build_id 解析和保守比较、统一 version_line；CLI 与独立内核共用库常量，内核启动日志带 build_id。无账本、调度、迁移、安装或面板改动。effective_override 尚未接环境变量，后续里程碑接入。

构建次序取祖先提交数，时间只显示。worktree 使用实际 git-dir/common-dir；无 reflog 且松散引用不存在时仍监视该路径。存在 shallow 文件时也监视它，避免解除浅克隆后仍缓存未知标识。git 不可用或计数溢出均降级为未知来源并警告。

此前没有审计轮，无需迁移审计退回检查。

## 命令与结果

验收命令：

`cargo build -p automed && cargo test -p automed --lib build_id && cargo test -p automed --test build_rerun && cargo test -p autome-cli --test cli build_id`

最终输出：`.autome/output/m01-acceptance-final.txt`。结果：21 个不同用例全部通过，失败 0、忽略 0；库 15 + 构建集成 4 + CLI 2。上一实现轮不存在，本任务新增用例基线为 0，增量 +21。相对设计原定 20 个增量 +1，来自 build_fallback_is_unknown_without_usable_git，将缺 git、非仓库、浅克隆、溢出和命令错误纳入项目测试。未减少或跳过原有测试；定向命令过滤的其它用例不计入本轮验收总数。

初次验收 20 个通过后发现共享模块在库测试编译中有 dead_code 警告，已通过实际调用 compute/repository_watch_paths 验证编译身份与真实路径消除；补充降级用例后最终为 21 个，无编译警告。

负向对照：临时将 watch_paths 的 `exists(&loose) || !has_log` 改为 `exists(&loose)`，运行 `cargo test -p automed --test build_rerun build_rerun_follows_a_packed_ref_without_reflog`。退出 101，1 个用例失败（构建仍为父提交标识）；输出 `.autome/output/m01-negative-packed.txt`。恢复后重新运行完整验收，21 个通过。此为本轮主动反例，不是此前审计回归；检查已在项目测试体系。负向对照重复执行已有用例，不增加不同用例总数。

执行了修改文件的 rustfmt 与 git diff --check。未运行全量门禁、宿主快检或无关测试，留给宿主。真实二进制 --version 均指定临时 AUTOME_HOME；构建集成仅使用临时仓库和临时 target。

## 标待审之前的自审清单

### 1. 情形表逐行

- 无记录：missing_record 验证 git/unknown 均 Advance（应写）。
- 文本相同：roundtrip 对两种来源验证 Same（不写）。
- git 祖先数更大：count_beats_time 验证 Advance，时间倒退不影响。
- git 祖先数更小：同测试验证 Older。
- 同数不同 sha：equal_count_different_sha 双向 Incomparable。
- git 对未知：unknown_is_older 验证 Advance。
- 未知对 git：同测试验证 Older。
- 两个不同未知：different_unknowns 验证 Older。
- 记录解析失败：corrupt_record 验证 Older；两份相同非法文本也不放行。
- 重跑落法：worktree_paths 验证 HEAD、logs/HEAD、packed-refs、松散引用；无松散引用但有日志时用日志覆盖，无日志时仍输出缺失引用；detached_paths 验证游离 HEAD。真实 cargo 集成验证 worktree、packed-ref 无 reflog、同秒提交及时间倒退。
- 输出：CLI 测试独立以 git 推导预期标识，比较两个二进制；源码检查确认 automed core starting 加入 build_id。

### 2. 设计点名的失败分支

build_fallback_is_unknown_without_usable_git 对缺 git、非仓库源码包、浅克隆、祖先数 10000000、git 返回非零各运行构建脚本，均成功退出、产生警告且输出可解析 unknown 标识。malformed 覆盖解析失败；override_only_older 覆盖非法或更高覆盖值拒绝。授权、外部服务、超时不适用：M-01 不涉及这些调用。

### 3. 全部状态迁移

无运行时状态机，不适用账本/面板状态迁移（后续里程碑）。构建身份迁移已执行：worktree 父→子、packed-ref→新松散引用、同秒父→子、时间倒退父→子；每次构建与独立 git 结果一致，父标识被 verdict 判旧。两种来源和记录状态组合均按上一节覆盖。

### 4. 输入域边界

malformed 验证空串、短计数、七位前导零、0000000、9999999、八位溢出、空版本、非 ASCII、换行、sha 大写；降级测试验证生成端 10000000 不生成非法 git 标识。utc_boundaries 验证 epoch、epoch 前一秒、2024 闰日、跨年两端。roundtrip 验证原文保留。时间解析依设计只检查数字形状，不额外定义日历有效性约束。effective_override 验证相同、更旧、更高、非法和未知来源。

## 结论

M-01 可标待审，等待独立审计，未标已完成。证据绑定提交前工作区；提交改变 HEAD 后标识会更新，审计需重新构建取证。
