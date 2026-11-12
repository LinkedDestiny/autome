# 实现 #9：默认分支 rebase 集成

## 范围与状态

简报无开放里程碑，M-01..M-08 已被审计关闭，上一轮没有退回缺陷。本轮按用户追加指令处理 rebase，不新增里程碑、不重新关闭或改写既有审计结论。implementation-round 更新为 9/40，current-milestone 与 next-action 保持「无」。本文件挂在最终集成 M-08 下作为本轮证据。

## 冲突处理

`git rebase main` 到本地默认分支 `0c57ff9`，逐次解决冲突并 `GIT_EDITOR=true git rebase --continue`，已完成。未 fetch、merge、推送或切换分支。`git merge-base --is-ancestor main HEAD` 返回 0。

- config.rs 同时保留 gate_concurrency、gate_run 初始化与两侧配置测试。
- scheduler.rs 同时保留队列准入与 FixGate repro-only 标记；超时简报逻辑随渲染函数迁移到 brief.rs，既有超时测试改用新函数路径。
- launcher.rs 同时保留超时分类文案与 before_merge 修复提示。
- 协议保留主分支 S-1..S-3 和本分支 G-1..G-6，seed 合并双方用例，测试合成版本统一 v90。文档保留队列、准备与 before_merge 说明。
- 两份 before_merge 金样同步主分支审计/复核写入边界；未改旧模式金样。
- 旧模板兼容测试显式先选出 each_round 模板，构造无分段的旧模板输入，保留原逐字断言。MainCheck 实跑次数测试显式配置 reuse_for_main_check=false，保留原执行次数断言，避免主分支新增默认复用改变测试前提。

## 验证与红绿

原始输出全部在 `.autome/output/r9-*`，本文件仅记判据。

- 配置：`cargo test -p autome-domain config::`，37 通过（总库存 407，筛掉 370；集成 1 个未选中）。前轮记录 domain 总库存 394，本次增加 13 来自主分支，本轮未新增测试；原记录没有 config:: 子集基线，不能把 37 与 394 作同口径比较。
- 库：`cargo test -p automed --lib`，721 个，720 通过、1 失败（旧模板夹具）；相对前轮 675 增加 46，来自 main 集成。修正夹具后 `cargo test -p automed --lib launcher`，65 全通过（覆盖失败用例，前轮 63，主分支增加 2）；另单跑 legacy 用例 1 通过。未宣称修复后重新跑过整个 721 用例集。首次编译曾因超时测试的 brief 模块路径未限定而失败，已改 crate::brief，后续库测试完成编译。
- 提示词：`cargo test -p automed --test prompt_rendering`，先 22 通过、2 失败；核对差异为 main 新增写入边界后，仅刷新 before_merge 金样，再跑 24 全通过。相对前轮 20 增加 4，来自 main。
- e2e：`cargo test -p automed --test end_to_end before_merge`，11 个，10 通过、1 失败，失败输出为实际 merge_gate 一次、原期望另有 main_check；本过滤集合与本分支先前新增 11 个一致，差 0。修正显式复用配置后单跑失败用例（结果见下）。当前 e2e 总库存 132，前轮 123，main 增加 9。
- `cargo run -q -p automed -- protocol eval protocol/`：8 项通过、0 项失败、1 项历史静态警告；20 个 eval 用例，相对前轮 19 增加 1（main 的 audit-keeps-project-files-unchanged），CHANGELOG 14 条。
- `cargo fmt --all -- --check`、`git diff --check` 均通过；源码、协议与两份产品文档扫描无冲突标记。
- 未运行完整宿主门禁、桌面/Web 测试、付费模型评估或人工验收；这些结果不由旧 SHA 的门禁证据替代。

## 四项自审

1. **情形表逐行**：保留 each_round 默认与 before_merge 两种配置，配置 37 例通过；需求跳过 Gate、Bug FixGate 只复现、失败修复提示、审计当前 SHA 证据由 before_merge e2e 及库测试覆盖；主分支队列和超时测试包含在 720 个通过用例内。无新增设计情形。
2. **失败分支**：修复门禁失败返回实现、无效 Bug 修复、旧 SHA 结果不冒充、超时与普通 exit 124 分类均执行对应已有测试。拒绝授权、外部服务不可达不适用：本轮没有新增外部接口；读盘失败等既有库用例未改。
3. **全部迁移**：没有新增迁移；需求实现到审计、MergeGate 失败到修复及重新合并、Gate 中切换模式、Bug FixGate 到复核/修复由过滤的 11 个 e2e 执行；main 队列准入仍在实际启动前，repro-only 标记仅在获准启动时写入。
4. **域边界**：配置默认/显式模式、无门禁、有门禁、旧模板无标记、新模板有标记、命令中花括号由配置和 launcher 测试覆盖；日期、数值新增上下限不适用，本轮未扩展输入域。

没有上一轮审计发现缺陷的临时检查需要搬运；本轮红绿均来自项目现有测试。里程碑表保持原审计结果，未将任何里程碑标为已完成。

最终补充：`cargo test -p automed --test end_to_end feature_before_merge_two_milestones` 修正后 1/1 通过，耗时 86.84 秒，覆盖两里程碑、实跑 MainCheck 与反向切回 each_round。与首次其余 10 个通过结果合计，11 个相关 e2e 均已有通过证据；未重复运行整组。最终格式与 diff 检查通过。
