# 实现 #5：默认分支 rebase 冲突修复

## 范围与修复

简报、设计里程碑表与上一审计一致：三个里程碑均已由审计关闭，没有开放项。本轮处理用户追加的 rebase 请求，不虚构里程碑、不重开已通过项，不由实现轮关闭任何里程碑。状态轮次为 `5/15`，H-01 仍待人工确认。上一审计通过，无退回缺陷，不适用缺陷检查原样搬入与红绿复现要求。

- 默认分支 `origin/HEAD -> origin/main`，本地 main 为 `9626bc1b6a9fff4de6f36967b5e3578d9ab430cd`；执行 `git rebase main`，未 fetch、未推送、未 merge。
- 逐个解决三个文件尾新增测试的冲突：`crates/automed/tests/end_to_end.rs`、`crates/autome-cli/src/render.rs`、`apps/web/test/write-gate.test.ts`。补齐两侧函数闭合与测试属性，完整保留 main 与本任务的测试。
- 每处 `git add` 后 `GIT_EDITOR=true git rebase --continue`；31 步完成，重放后 HEAD 为 `3548402efc0209abdd52ab472d218ebf7ba0e5a0`。`git merge-base --is-ancestor main HEAD` 返回 0。
- 与 main 对比，端到端文件 +61/-0，网关测试 +26/-0；render +13/-1，仅转换提示及一个测试。冲突解决没有删除上游测试或改动产品行为。

## 四项自审

1. **情形表逐行**：检查三个冲突文件相对 main 的差异，保留上游门禁、CLI 队列与 T-37 网关情形；本任务默认改建、start、重复改建拒绝、附件字节比较与提示断言均保留。执行结果见下节。
2. **失败分支**：现有 Rust 测试覆盖非 Bug、非法状态、缺失/非法参数、任务不存在、附件缺失、停会话读目录失败、事务三个写步骤失败回滚；网关覆盖缺字段、空串、错误类型与错误方向。未引入外部服务，外部服务不可达不适用。
3. **全部迁移**：保留并运行原 not_a_bug → cancelled、新需求 draft/queued 与非法状态不变的测试；未新增状态或通道。main 新增的门禁队列迁移用例未删减。
4. **输入域边界**：网关测试执行未传/true/false/null、1/512/513 字符、空串与错误类型；原始 brief 字节和单附件由 Rust 测试覆盖。日期、闰日、跨年不适用：改建接口没有日期输入。

## 验收与用例数

- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、Web lint/typecheck/build 均通过。
- `npm test --prefix apps/web -- test/write-gate.test.ts`：44/44，通过总数比上一审计 36 增加 8；新增来自 main 合入的 T-37 等网关测试，本轮无用例增删。
- `cargo test --workspace` 首跑退出 101：automed 单元 729 passed / 1 failed；失败为上游 `scheduler::tests::gate_queue_upgrade_collects_two_old_jobs_without_killing_them` 第 225 行，实际运行数 1、预期 0。单独执行 `cargo test -p automed --lib gate_queue_upgrade_collects_two_old_jobs_without_killing_them` 重试 1/1 通过。首次失败仍保留，不能称全量全绿；该源码与 main 一致，无证据归因到改建实现。
- `npm test --prefix apps/web` 出现 live/server 启动用例失败或跳过，以及 watcher `EMFILE`，随后长时间没有进展，主动中止，退出 130；未取得完整结果汇总，不把部分执行当作全量证据。需宿主复验。
- 本轮未重跑整个 `sh scripts/ci-local.sh`、Electron DOM 与 `npm ci`：使用现有依赖执行受影响套件及上述静态检查；完整宿主门禁仍未取证，沿用 next-action 要求。没有把环境/运行问题标成协议失败。
- Web 用例总数：`npx vitest list --json` 当前发现 371 例；上一轮执行汇总为 365，表面差 +6，但这不是同口径的实跑差。源码对比上轮 HEAD，TS 顶层测试净增 20（任务卡 HTTP 4、生命周期 2、gateway 2、read-gate 4、write-gate 8），另新增一个独立 mjs 类型回归用例。本轮未删除测试；发现列表与历史实跑汇总的 14 例口径差尚未核实，不能掩盖成全量通过，宿主须重新取得完整总数。
- 首跑中断后的剩余 Rust 目标补跑：`cargo test -p automed --test end_to_end --test prompt_rendering --test real_cli --no-fail-fast` 通过，分别 133/133、24/24、7/7；另补跑 automed bin 与 workspace doc 均通过（0 例）。第一次误指定不存在的 `driver_lock` test target 被 Cargo 拒绝，纠正为上述三个实际集成目标后执行。
- 合并首次执行与未执行目标的补跑，Rust 共 11 个目标、1352 个唯一用例：1350 passed / 1 failed / 1 ignored；失败那 1 例单独重试通过，不重复计入总数。比上一轮 1222 总数增加 130，全部来自 main：CLI +8、domain +23、automed 单元 +71、端到端 +20、prompt_rendering +8；源文件 `#[test]` 净增计数同为 130。本轮无新增/删减用例。
- 本任务 task_convert 共 13 例均已在上述运行通过（内核 10，CLI 3），与上一审计差 0。核心附件端到端及全部冲突两侧测试均通过。

## 结论与输出

rebase 完成，转换功能回归通过，三个里程碑保持审计已有的关闭状态；本轮没有待审/开放里程碑可推进。完整门禁未取得全绿证据，需宿主复验 Rust 偶发失败、Web 全量及 Electron DOM，并由用户确认 H-01。

原始输出与计数明细在 `.autome/output/impl5-*`：rebase/continue、conflict-resolution.diff、cargo、retry、integration、bin、doc、gateway、web、web-list、test-deltas、web-case-deltas、fmt、clippy、lint、typecheck、build。证据正文不转存原始输出。
