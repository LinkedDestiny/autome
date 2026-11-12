# M-02 · 实现轮 3/10

结论：e2e 实现完成，Rust 与插件检查通过；完整门禁受当前运行环境阻塞，M-02 保持开放，不标待审、不增加 reopen。上一轮审计通过 M-01，没有发现实现缺陷所用的检查需要迁入，因此无该类红绿回归迁移。

## 改动

仅修改 `crates/automed/tests/end_to_end.rs`：新增 `bug_loop_prompts_name_the_card_attachments_and_doc_refs`。在 World 仓库外创建 crash.log 并 canonicalize，按真实 CLI 的绝对路径形态传入 task.create，同时提交 docs/notes.md 并传入 doc_refs。执行定位、修复、复核到 await_merge，逐个读取真实 prompt 文件，断言附件副本相对路径、参考路径、无源目录泄漏；检查附件副本内容及参考文件存在。既有无输入流程补三个 prompt 的无标题、无占位符断言。生产代码、存储、状态机与模板没有新增改动。

## 命令、结果与基线差

原始日志均位于 `.autome/output/`，不复制到任务文档。

- `cargo test -p automed --test end_to_end bug_loop`：退出 0，总数 15，15 通过、0 失败、0 忽略。相对上轮/设计基线 14 增加 1，唯一新增即本轮输入 e2e；其余增强断言不增加用例数。日志 `m02-bug-loop.txt`。
- `cargo test --workspace`：退出 0，总数 1203，1202 通过、0 失败、1 既有忽略。相对 M-01-r2 证据的 1202 增加 1，来自同一新增 e2e（端到端 101 → 102）；没有删除用例或增加忽略。日志 `m02-workspace.txt`。
- `sh scripts/ci-local.sh`：未全绿，最终人工中止退出 130。Format、Clippy、Rust 全量、Contract、Web lint、Web typecheck 通过。Rust 总数同为 1203（1202 通过、1 忽略），相对上轮 +1。Desktop 总数 105（104 通过、1 失败），相对上轮差 0；失败为 `electron dom harness: real-DOM assertions against the actual renderer`，子进程 status=null。Web test 运行约 395 秒后仍未给出最终用例汇总，已出现实时服务套件跳过及服务启动测试失败，随后中止；完整门禁在进入 Web build 后终止。日志 `m02-ci.txt`。Web 本轮完整总数未取证，上轮实现轮同样没有取得完整总数，不能计算差值，更不能将缺失层当作通过。
- `npm run build --prefix apps/web`：独立执行退出 0，日志 `m02-web-build.txt`；不替代完整门禁通过。
- `sh plugins/autome/tests/run.sh evals`：退出 0，总数 5，5/5 通过；相对既有 5 项差 0，无插件改动。日志 `m02-plugin.txt`。
- `git diff --name-only 83c80e9 -- plugins/ .autome/ apps/ docs/bug-loop-的定位-修复-复核/protocol/ crates/automed/src/scheduler.rs crates/automed/src/dispatch.rs crates/autome-cli/`：退出 0、输出为空；`git diff --check` 通过。

## 标待审之前的自审清单（已执行；因门禁阻塞未标待审）

1. **情形表逐行**：设计 M-02 的带附件+参考输入，经 task.create 到三份 prompt 均通过；无附件且无参考，既有 merge 流程三份 prompt 的新增断言通过；复制文件确实存在并保持内容，参考文件存在，均通过；绝对源路径不泄漏，三个步骤均通过。范围核对为空。完整门禁一行不通过，不能报验收完成。
2. **失败分支**：本轮没有新增授权、读盘、超时或外部服务调用分支，上述分支对本轮渲染改动不适用。空输入通过既有流程的新断言；设计指向的缺占位符和错误绝对路径行为，由已运行的 launcher/金样及新 e2e 守住，本轮未做生产代码缺陷注入。运行环境实际失败（Electron 启动、网络监听）另列如下，没有跳过或弱化测试。
3. **全部迁移**：本轮输入的三个使用点 locate → fix → fix_review 各读取一次实际 prompt，并断言最终 await_merge、会话数 3；无输入既有用例断言 locate → repro_check → fix → fix_gate → fix_review → rebase → merge_gate → await_merge → done。15 个 Bug 用例全部通过，包含复现未失败退回定位、修复仍失败退回修复、复核拒绝、崩溃重试、无法复现/补充信息、非 Bug 处置、预算耗尽/扩展与多仓库路径。没有修改状态机或新增迁移。
4. **输入域边界**：空列表由无输入 e2e 覆盖，单附件/单参考由新增 e2e 覆盖；绝对路径经 canonicalize，非裸文件名。空格文件名、仅一种输入及花括号用户文本由全量中的既有 launcher 测试覆盖。没有日期、数字上限等输入域声明，前导零、闰日、跨年、最大值不适用。

## 环境证据与后续

- 最小 Electron `--version` 的 Node spawnSync 探针结果为 status=null、signal=SIGABRT、stdout/stderr 空（`m02-electron-probe.txt`），故桌面失败不能描述为 DOM 断言已经通过。
- 最小 Node net.Server 在 127.0.0.1 随机端口监听返回 EPERM（`m02-listen-probe.txt`），当前环境确实不允许本地监听。Web 失败的全部逐项根因尚未取得，不能断言全部都由该限制导致。
- 未改桌面/Web、未提权、未修改门禁；未取得完整门禁全绿，保持 M-02 开放。后续在允许 Electron 及本地监听的环境原样运行 M-02 验收链，核对 Web 完整测试总数后再标待审。这不是协议失败。
