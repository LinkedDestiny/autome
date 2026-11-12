# 实现 #10：修复合并前门禁 Clippy 失败

## 范围与根因

本轮按用户注入的门禁失败指令修复，不推进新里程碑。现有 M-01..M-08 均已由审计关闭，保持里程碑表原样；implementation-round 为 10/40。证据继续挂在最终集成 M-08 下，最终门禁以 Autome 对本轮提交的重跑结果为准。

`launch_session` 的 `resolved` 参数已经是 `&ResolvedConfig`，调用 `brief::gate_evidence` 时再写 `&resolved` 形成不必要的二重引用。Clippy 在 scheduler.rs:2671 报 `clippy::needless_borrow`，被 `-D warnings` 提升为错误。只删除这一处 `&`，不改变配置、事件、简报内容或状态机。全目标 Clippy 检查同类问题，不添加 allow、不修改门禁或测试断言。

上一轮审计没有退回实现缺陷，没有临时判别检查需要搬运。本次发现问题的 `cargo clippy --all-targets -- -D warnings` 已逐字存在于项目 `scripts/ci-local.sh`，持续承担回归检查；不为编译期 lint 新增无法检测该问题的运行时测试。

## 命令与红绿证据

- 红：`sh scripts/ci-local.sh > .autome/output/r10-gate-red.txt 2>&1`，修改前启动，Clippy 报上述错误；Rust 测试编译完成后才修改源码，原运行继续完成。退出码 1，391 秒；Clippy、Desktop、Web test 失败，其余步骤通过。
- 绿（直接复现项）：`cargo clippy --all-targets -- -D warnings > .autome/output/r10-clippy-green.txt 2>&1`，退出码 0，3.27 秒。
- 同命令复验：`sh scripts/ci-local.sh > .autome/output/r10-gate-after-fix.txt 2>&1`，退出码 1，469 秒。Format、Clippy、全部 Rust 测试、Contract、Web lint/typecheck/build 通过；Desktop、Web test 仍失败。不能声明全量通过。
- Desktop：两次均为 DOM harness 启动后 SIGABRT；本会话未获得真实 Electron DOM 通过证据。Web：两次均有 `listen EPERM: operation not permitted 127.0.0.1` 与 watcher EMFILE；独立的 Node `net.createServer().listen(0, "127.0.0.1")` 探针同样返回 EPERM（`.autome/output/r10-listen-probe.txt`），证明监听限制不依赖业务代码。宿主提供的旧提交报告中这两步通过，本轮不通过修改测试来掩盖本地限制。
- `git diff --check` 通过。最终源码改动只有删除一个多余引用；没有修改 `.autome/` 配置、测试、门禁脚本或其它 worktree。原始日志仅写入获准的 `.autome/output/`。

## 用例总数与基线差

两次本地完整门禁均收集 1828 项：Rust 1339（1338 通过、1 个既有忽略）、Desktop 105（104 通过、1 失败）、Web 384（347 通过、18 失败、19 跳过）。修复前后总数差 **0**，本轮新增/删除测试均为 0。Web 的 19 个跳过来自启动失败后的套件未执行，不是新增 skip；没有把它们记为通过。

与上一轮 r9 的同口径库存比较：domain 407、automed lib 721、end_to_end 132、prompt_rendering 24，分别差 0；本轮 lib 全部 721、e2e 全部 132 实跑通过，补齐 r9 仅对子集修复后取证的覆盖。Web 384 与用户提供的宿主基线差 0。r9 未提供其它套件的全量总数，故不虚构上一轮 1828 的历史基线；1828 是本轮原样失败运行与修复后运行的同口径对照。Rust 其余为 CLI 单测 33、cli 集成 14、real_cli 集成 7、既有集成忽略 1。

## 四项自审

1. **情形表逐行**：本轮只改变传参表达式。设计 §6.2 的无门禁、each_round、before_merge 同 SHA、before_merge 无结果/旧 SHA/无 head 均仍由原 `gate_evidence` 分支处理，未改分支或参数值；对应现有 brief 单测与金样纳入完整门禁。
2. **失败分支**：本轮直接失败分支为 Clippy 拒绝多余引用，已先红后绿。设计 §6.2 的读不到 head 仍通过 `.ok().as_deref()` 传 None；已有测试纳入门禁。拒绝授权、外部服务不可达等不适用：未引入 I/O 或接口。
3. **全部迁移**：不适用本次代码改动：删除多余引用不改变状态或事件。原需求/Bug 转换、失败回路与模式切换回归由完整 Rust 套件验证。
4. **域边界**：无新输入域；现有 None、不同 SHA、相同 SHA 和门禁模式边界测试由完整门禁验证。日期、数值最大值不适用：本次改动不解析输入。

未执行付费模型行为评估或人工验收；本地门禁结果不替代提交后宿主验证。
