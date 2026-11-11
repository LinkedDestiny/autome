# project add 初始化提交卷入用户未跟踪文档

status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p automed dispatch::tests::project_add_init_commit_preserves_untracked_user_docs -- --exact
根因: init_commit 将整个 docs 目录交给 git::commit_paths，git add 递归暂存用户原有的未跟踪文档并纳入初始化提交。
复核: 通过
repos: autome-v2

## 现象

在已有 README 提交的仓库中，预先创建未跟踪的 `docs/draft.md`、`docs/research/notes.md` 和 `personal.txt`。执行添加项目并跳过 onboarding 后，前两个文件进入 `chore(autome): initialise project` 提交，只有 `personal.txt` 仍未跟踪。

任务卡期望初始化提交只包含 Autome 生成的文件，用户原有文件保持未跟踪；实际行为违反这一边界，属于 Bug。

## 复现

- 定位基线：`c7394d19d414be04bd0d612be9c46066cf67c5b8`。
- 从本任务 worktree 根目录执行状态块中的命令。本项目是单 Git 仓库（包含 Cargo workspace），并非需要切换成员 Git 仓库的 Autome 工作区项目。
- 前提：可用的 Git、Rust/Cargo，以及已缓存的 Cargo 依赖。使用 `--offline`，不访问网络，不启动 daemon、tmux 或真实 Agent。
- 回归测试位于 `crates/automed/src/dispatch.rs` 的 `dispatch::tests::project_add_init_commit_preserves_untracked_user_docs`，使用隔离临时仓库和内存 Store，结束后自动清理。
- 测试调用真实 `project.add` 和 `project.onboarding.skip` dispatch 路径，与 CLI `--skip-onboarding` 的调用顺序一致；不模拟 Git 提交。没有执行已安装的 CLI 二进制。
- 测试确认草稿最初未跟踪，且添加项目前后暂存区为空；随后验证初始化提交确实生成、包含脚手架、`.gitignore` 和 `AGENTS.md`，再验证用户文件内容和未跟踪状态。缺少 Git 会失败，不会跳过。

连续两次执行均编译成功并以退出码 `101` 失败：`0 passed; 1 failed; 0 ignored`。失败来自业务断言，而非超时或环境错误：

```text
user file docs/draft.md must remain untracked; init commit files:
.autome/rules/README.md
.autome/skill/run_session.sh
.gitignore
AGENTS.md
docs/draft.md
docs/research/notes.md

untracked:
personal.txt
```

原始日志：`.autome/output/init-regression-1.log`、`.autome/output/init-regression-2.log`，不纳入提交。

## 根因分析

1. `crates/autome-cli/src/main.rs` 的 project add 分支在跳过向导时发送 `project.onboarding.skip`。
2. `crates/automed/src/dispatch.rs` 的 `onboarding_step` 在首次进入终态时调用 `init_commit`。
3. `init_commit` 的路径列表包含 `.autome`、`.gitignore`、`AGENTS.md` 和整个 `docs`，只检查路径是否存在，没有区分文档是否由 Autome 生成。
4. `crates/automed/src/git.rs` 的 `commit_paths` 将该列表传给 `git add --`，然后提交暂存区。因此 `docs` 下未被忽略的用户文件也被递归暂存并提交。

对照证据：添加项目后暂存区仍为空，排除了脚手架初始化另行暂存草稿；测试初始暂存区为空，排除了已有暂存内容混入；目录外 `personal.txt` 保持未跟踪，说明不是全仓库无差别暂存。

`init::InitReport::paths_to_commit` 已能列出初始化改动路径，并由 `project.add` 返回为 `pending_commit`，但当前 `init_commit` 没有使用这一来源。后续修复应限定 Autome 实际生成的具体文件，同时保留合法 onboarding 产物；本轮未实现修复，也未把预先暂存内容等相邻问题纳入本回归范围。

## 修复说明

第 1/3 轮仅修改 `init_commit` 的暂存路径：移除整个 `docs` 目录，复用 `ONBOARDING_FILES`，明确纳入 `docs/agent-project-profile.md` 和 `AGENTS.md`；保留 `.autome` 脚手架及 `.gitignore` 的原有行为。这样合法 onboarding 产物仍可提交，其他用户文档不再被递归暂存。不涉及预先暂存内容等相邻问题。

定位轮回归测试未修改、删除、跳过或弱化。同一个状态块复现命令在修复前失败（业务断言，1 failed），修复后成功（1 passed，0 failed）；日志分别为 `.autome/output/fix-1-before.log`、`.autome/output/fix-1-after.log`。

配置门禁 `sh scripts/ci-local.sh` 已运行，退出码 1：Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 通过；Web test 未通过（3 个测试文件失败，11 failed、198 passed、16 skipped），日志报告 `listen EPERM: operation not permitted 127.0.0.1` 和 `EMFILE: too many open files, watch`。当前执行环境阻塞 Web 服务监听，不能据此声称项目门禁通过；需由宿主在允许本机监听的环境重跑。未修改测试或门禁以绕过失败。完整日志为 `.autome/output/fix-1-gate.log`，不纳入提交。`git diff --check` 通过。

## 复核意见

通过（第 1/3 轮复核，未改动回归测试与产品代码）。

- 复现命令独立重跑：修复提交上 `1 passed; 0 failed`；`cargo test --offline -p automed` 全量 602 + 77 + 16 + 7 个测试均通过。
- 差异（be7a2fc..c4a9a8c）：产品代码仅 `dispatch.rs` 的 `init_commit` 一处，9 行；回归测试文件未触碰，未删除、跳过或弱化。
- 根因成立：旧路径列表含整个 `docs` 目录，`git add --` 递归暂存用户未跟踪文档；修复后只暂存 `.autome`、`.gitignore` 及 `ONBOARDING_FILES`（`docs/agent-project-profile.md`、`AGENTS.md`），与根因一一对应。
- 回归测试有意义：真实 dispatch 路径，先断言草稿未跟踪且暂存区为空，再断言初始化提交含脚手架与 onboarding 产物、用户文件仍未跟踪；修复前以业务断言失败。
- 范围最小，保留合法 onboarding 产物提交。
- 修复说明中的 Web test 失败为 `listen EPERM`/`EMFILE` 沙箱环境问题，与本次 Rust 改动无关；宿主门禁需在允许监听的环境重跑，本轮未能复验该项。
