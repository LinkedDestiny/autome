# 每轮快检：`[gate] quick = "…"`，`before_merge` 下实现轮 / 修复轮之后只跑一条几秒钟的静态检查

## 背景

T-61 让项目可以配 `[gate] run = "before_merge"`：完整门禁（本仓库是 `sh scripts/ci-local.sh`，约 3 分钟）每个任务只在合并前跑一次。代价是格式、lint、类型检查这类**几秒就能跑完**的问题也要等到合并前才暴露，暴露后要多走「实现 → 审计 → 复盘 → rebase → 合并前门禁」一整圈。门禁关闭 / 合并前模式下已经出过两次：

- T-56 带进 main 的 `Web typecheck` 失败（14 处隐式 any），靠另开 T-65 修；
- T-61 自己的 `clippy::needless_borrow`（M-04 引入，经过 5 个审计轮没人发现），合并前门禁第一次运行才拦住，多花实现 #10、审计 #10、复盘 #3 和两次完整门禁，约 40 分钟。T-61 的复盘 L-07 记录：同一条 `cargo clippy` 在会话里只要 3.27 秒。

把这类检查交给 prompt 去「提醒会话自己跑」不可靠（L-07 的现象就是每一轮都写「由宿主门禁负责」）。宿主应当自己跑，像完整门禁一样用退出码说话。

## 要做什么

1. 项目配置 `.autome/config.toml` 的 `[gate]` 新增可选键 `quick`（字符串，`sh -c` 在任务 worktree 里执行）和可选键 `quick_timeout_minutes`（缺省 5，范围与 `timeout_minutes` 的校验同一风格）。没配 `[gate] command` 时单独写 `quick` 报错。全局配置不认这两个键。
2. `run = "before_merge"` 且配了 `quick` 时：
   - 需求 Loop：实现轮结束后进 `Gate` 节点，但跑的是 `quick` 命令；通过 → 去审计（或审计关闭时的下一步）；失败 → 照现有 `Gate` 失败路径退回实现轮，prompt 带命令与日志尾部。
   - Bug Loop：FixGate 跑 `( 复现命令 ) && ( quick 命令 )`。
   - MergeGate 不变：完整门禁命令（Bug 是复现命令 + 完整门禁）。完整门禁是否包含 quick 的内容由项目自己决定，Autome 不拼接。
3. `run = "each_round"`（缺省）时 `quick` 不生效，行为与今天逐事件相同；配置解析通过，`autome doctor` / 配置校验不报错（可在 `config.get` 的 `violations` 之外给一句说明，不强求）。
4. 快检与完整门禁共用宿主任务队列（T-47 的 `[gate] concurrency`）、同因失败熔断、126/127 处理、日志落盘位置（`.autome/output/gates/<任务>/`）、`AUTOME_GATE_LOG` / `AUTOME_GATE_NODE` 环境变量；另导出 `AUTOME_GATE_KIND=quick|full`，让脚本能区分。
5. 读者能分清跑的是哪一种：`gate.started` / `gate.finished` 事件带 `kind: "quick" | "full"`；`autome run` 的行写「快检开始 / 快检通过 / 快检未通过」；`autome status <任务>` 的门禁行与 `task.get.gate` 标明种类；审计 / 复核简报的「宿主门禁」段写明「本轮宿主只跑了快检：<命令>，退出码 N」，不让审计把快检当成完整门禁的证据。
6. prompt 的 `before_merge` 段（T-61 引入的 `<!-- gate-run: before_merge -->` 分段与 `{gate_line}`）在配了 `quick` 时说明「宿主会在本轮结束后跑快检 `<命令>`，失败会退回」，没配时保持现有文案。协议正文有改动就按 `protocol/CHANGELOG.md` 的规矩发新版本（独立的 `## protocol/vN` 标题）。
7. README Host gate 一节与 `docs/development/technical-design.md` 对应小节各补一段：两级门禁的取舍与推荐（快检放格式 / lint / 类型检查，控制在 1 分钟内；测试留给完整门禁）。
8. 本仓库的 `.autome/config.toml` 不在任务里改；合并后由用户（或其授权的操作者）加：`quick = "cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web"`。验收里请实测这条命令在热缓存下的耗时并写进证据。

## 不做什么

- 不改 `each_round` 下的任何行为。
- 不自动从完整门禁里拆出快检，不解析 `ci-local.sh`。
- 不做「按改动文件选择快检命令」；一条命令，项目自己写条件。
- 不做跨任务合并队列。

## 验收标准

1. 单测（`autome-domain`）：`before_merge` + `quick` 下，实现轮完成 → `Gate`；`Gate` 通过 / 失败的转换与 `each_round` 下 `Gate` 的转换相同；没配 `quick` 时与 T-61 现有行为逐条相同（现有用例不改就通过）。
2. e2e（需求）：`before_merge` + `quick`，两个里程碑的任务从创建到合并：快检脚本被调用 2 次（每个实现轮一次），完整门禁脚本恰好 1 次；让快检第一次失败 → 退回实现轮且 prompt 带快检尾部 → 修好后继续；事件里 `kind` 正确。
3. e2e（Bug）：FixGate 的命令含复现命令与快检、不含完整门禁；MergeGate 含复现命令与完整门禁。
4. 解析：只有 `quick` 没有 `command` 报错；`quick_timeout_minutes` 越界报错；`save_project` 不改写用户的 `[gate]`；`each_round` + `quick` 不跑快检。
5. 简报与 prompt 金样覆盖第 5、6 条文案；`autome run` / `status` 的渲染有单测。
6. `cargo fmt --check`、`clippy -D warnings`、`sh scripts/ci-local.sh` 全绿——**实现轮自己在会话里跑前两条并把退出码写进证据**（本项目当前是 `before_merge`，宿主到合并前才跑）。

## 参考

- `crates/autome-domain/src/config.rs`：`GateConfig`、`GateRun`、`gate_before_merge()`、`gates_each_round()`。
- `crates/autome-domain/src/task.rs` / `bug_loop.rs`：T-61 加的「`before_merge` 下跳过 `Gate`」「FixGate 只跑复现」的转换。
- `crates/automed/src/scheduler.rs`：`run_gate`（命令拼装、环境变量、排队）。
- `crates/automed/src/brief.rs`：`gate_evidence`、`gate_line_of_result`。
- `docs/门禁只在合并前跑一次-gate-run/lessons.md`：L-01、L-07。
