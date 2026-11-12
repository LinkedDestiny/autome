# 实现 #3：rebase 后回归证据

## 范围与状态

简报及权威设计均无开放里程碑：M-01、M-02 已分别由审计 #1、#2 关闭，上一轮没有实现缺陷。本轮按用户明确要求处理 rebase，不新建里程碑、不重新开放、不自行更改里程碑关闭状态。证据归于最后一个里程碑 M-02，并覆盖冲突涉及的 M-01 回归。状态块 implementation-round 更新为 3/10，current-milestone 与 next-action 均为无；整体 status 与已审计关闭的里程碑表同步为已完成。人工清单仍需合并前确认。

## 修复与命令

- `git rebase main`：默认分支由 origin/HEAD 确认为 main，本地目标 `1d06101`；没有 fetch、merge 或推送。
- 在重放 `5d94300` 时 `crates/autome-cli/tests/cli.rs` 冲突：默认分支在文件尾新增两个 task convert 用例，本分支在同处新增三个 run JSON 用例。保留 main 文件完整内容，再追加该提交新增的三个完整测试；`git add crates/autome-cli/tests/cli.rs` 后 `GIT_EDITOR=true git rebase --continue`，余下提交全部重放成功，最终代码 HEAD `de60238`。
- `git merge-base --is-ancestor main HEAD`：退出 0。用 Ruby 对比当前 cli.rs 是否以 `git show main:crates/autome-cli/tests/cli.rs` 的完整字节串开头：通过，证明默认分支原有测试全部原样保留。原分支的三个 run JSON 和一个 logs JSON 集成测试均保留且运行通过。
- 原始 rebase 输出：`.autome/output/r3-rebase.txt`、`r3-rebase-continue.txt`。
- 上轮审计为通过，无发现实现缺陷的判别检查需要搬入测试体系，因此红→绿复现不适用；没有伪造红例或新增重复用例。

## 验收命令与总数

- `cargo test -p automed --lib events_since`：4 个通过，较前轮 4 个差 0。输出 `.autome/output/r3-events.txt`。
- `cargo test -p autome-cli`：总数 61，41 单元＋20 集成，全部通过、无忽略。较审计 #2 基线 50（32＋18）净增 11：main 带入 9 个单元测试（gate_queue 三个、task_convert 一个、gate_timeout 四个、main_check_reuse 一个）和两个 task_convert 集成测试。本轮没有新增或删除测试。输出 `.autome/output/r3-cli.txt`，新增名称清单 `.autome/output/r3-added-tests.txt`。
- `sh scripts/ci-local.sh`：596 秒，最终非零。Format、Clippy、Rust 全量、Contract、Web lint/typecheck/build 通过；Desktop、Web test 失败。输出 `.autome/output/r3-gate.txt`。
  - Rust 总数 1375（1374 通过、1 个既有 ignored），较上一实现轮 1214 净增 161，均为 rebase 引入 main 的测试；本轮无测试增删。
  - Desktop 总数 105（104 通过、1 失败），较前轮差 0；electron dom harness 子进程 SIGABRT、exit null。
  - Web 总数 385（348 通过、18 失败、19 跳过），较前轮 301 净增 84，均来自 main。失败涉及 7 文件：eventbus-live、notifier-live、server、gateway-http、gateway-task-card、gateway-task-lifecycle、shell-live；原始错误为监听 127.0.0.1 的 EPERM、文件监听 EMFILE 及其启动等待超时。没有把环境失败当成通过，也没有修改范围外的 apps 或削弱测试。
- 完整宿主门禁及真实 Claude/Codex 人工验收仍需在允许 Electron、本地监听的环境复验。

## 自审清单（本轮不新增待审状态，仍逐项执行）

1. **情形表逐行**：通过本轮 CLI 全量逐项复验设计 §4：Claude 原始事件→对应 type/text/event；Codex→Hello!/用量文本；无可见内容事件→text null；非 JSON→event/type null、文本保留；空白行→跳过但保留行号；非法 UTF-8→替换字符；默认最新会话与指定前缀→对应原始流；普通输出与 ended-follow→结果一致；闭管→成功退出且 stderr 空。均由 stream_* 单元及 logs_json 集成通过。冲突涉及的设计 §3 也由 run_json 集成验证事件 seq/time/payload、task.head 未过滤、idle 收尾和空任务列表。
2. **失败分支**：logs_json 集成实际执行 ended 缺文件→码 1/stdout 空、目录代替文件→读盘失败码 1、running 缺文件→空输出、闭管→成功；run_json 用例覆盖非法 interval→码 2、tick 错误→JSON error、驱动锁占用→静默、SIGINT→无 idle 收尾。拒绝授权、外部服务不可达、超时不适用：本改动为本地日志/事件读出，设计未声明这些专属分支。
3. **全部迁移**：stream_line_boundaries 与 logs_json 集成验证 running＋缺文件、running＋完整行/半行、半行补齐换行、running→exited 后补无换行尾行、ended＋follow 读尽退出；run_json 验证 once、idle、SIGINT 三个退出方向。运行中持续 follow/Ctrl-C 的真实会话端到端仍是既有人工清单及 B-04，未取证；本轮未宣称补齐。
4. **输入域边界**：stream_text_and_input_boundaries、stream_line_boundaries 覆盖空串、空白、CRLF、单换行、无换行尾部、半个 UTF-8、非法 UTF-8、null/数值/数组/非字符串 type；全部通过。run_event 三单元验证任务与非任务主体、存储时间原值。前导零、闰日、跨年、最大数值的日期/数值解析不适用：本轮不新增输入解析域，时间和 seq 由账本提供并原样输出。

## 结论

rebase 完成，CLI 验收通过；已有里程碑审计状态保持不变。宿主门禁存在上述环境失败，真实会话人工项未取证。没有启动下一会话。
