---
name: autome-v2-simplify
description: autome-v2 仓库专用的证据驱动代码简化技能——两个 Rust crate（autome-domain 纯域 / automed I/O 与 JSON-RPC）加一个 Electron 外壳（apps/desktop，纯 node --test），懂这个仓库的验证命令、双份 allowlist、include_str! 协议 seed 与生成物路径。在本仓库做「简化 / 清理 / 去重 / 删死代码 / 找可删项 / 熵回收 / simplify / reclaim entropy / dead code / 这段代码太乱了帮我收一下」时使用。
---

# autome-v2 代码简化

目标是减少团队必须保持一致的事实、契约、状态与概念的数量，而不是删多少行。
一次大删除可能是错的，一次小删除可能卸掉整条维护义务。「没有可以安全删的」也是合法结论。
扫描器只产生候选；只有消费者、所有权、历史与验证四类证据才能为一次删除背书。

## 0. 仓库档案（本技能唯一仓库特定的部分，其余流程通用）

档案生成于 2026-09-19，基于 master @ 6e30d4c。档案里的每条事实都应能在仓库里找到出处；发现与代码不符时以代码为准并更新档案。

### 0.1 技术栈与目录

**Rust workspace**（`Cargo.toml`）：`edition = "2024"`，`rust-version = "1.93.1"`，`resolver = "2"`，
members 恰好两个：`crates/autome-domain`、`crates/automed`。

**Electron 外壳**（`apps/desktop/package.json`）：electron 43.2.0 是唯一 devDependency，无运行时依赖、
无打包器、无前端框架。测试是原生 `node --test`，CI 用 Node 22。

**Python**：`scripts/gen-eval-seed.py` 一个脚本（生成器，见 §0.5）。

顶层目录职责：

| 路径 | 职责 |
|---|---|
| `crates/autome-domain` | 纯类型与状态转移。README「Layout」：*"No I/O, no async, no SQLite."* 五个角色、配置叠加与校验、设计文档解析器、任务状态机。 |
| `crates/automed` | 一切碰外部世界的东西：SQLite、Git、会话启动器、调度器、环境探针、技能扫描、JSON-RPC 面。含 `[[bin]] automed`。 |
| `apps/desktop` | Electron 外壳，UI only。`main.js` / `preload.js` 是 Electron 入口；`src/*.js` 是抽出来的纯逻辑（可脱离 Electron 单测）；`renderer/` 是 `screens/` + `lib/`。 |
| `docs/adr` | 两份决策记录，见 §0.2。 |
| `docs/development` | 从 1.x 授权仓库镜像过来的需求与技术设计，见 §0.5。 |
| `docs/design` | 交互设计 HTML 稿，同为镜像。 |
| `docs/plans` | 计划、审计与提案，审计落盘处，见 §0.9。 |
| `legacy/loop-plugin` | 1.x Bash 插件，36 个跟踪文件，历史参照，不参与构建。 |
| `scripts` | `gen-eval-seed.py`、`package.sh`。 |

架构总览读 `README.md` 的「The shape of it」与「Layout」两节。

### 0.2 动手前必读

**本仓库没有 AGENTS.md、CLAUDE.md、CONTRIBUTING**。权威是 `README.md` 加 `docs/adr/`，
以及 `.github/workflows/ci.yml` 里成段的「为什么这样」注释——那些注释是红线的实际出处。

`docs/adr/0001-greenfield-independent-repo.md`（accepted，2026-09-14）三条原文：

- *"No build, test, or release step may reference a path inside the 1.x repo."*
- *"Every crate/file's provenance is either 'written for 2.0 against the plan' or 'copied from an
  explicitly named public source with license noted' — never 'adapted from 1.x source'."*
- *"Rust workspace has exactly two crates (`autome-domain`, `automed`) per plan §4, to avoid
  speculative store/harness/verifier/ipc crate splits before there is a second real consumer of
  any such boundary."*

第三条直接就是本仓库的简化原则，两个方向都要用：**没有第二个真实消费者就不拆边界**；
反过来，发现一个只有单一消费者的抽象层时，删掉它是在执行这条决策而不是违反它——
但先对照 §0.7，那里列了几个「单一消费者但有意保留」的例外。

`docs/adr/0002-v3-model-supersedes-the-09-13-plan.md`：判断某段代码是不是「被取代的旧计划残留」时读它。

`README.md`「Building」给了三条验证命令与各自的用例数，是 §0.3 的出处。
`docs/plans/2026-09-16-loop-v1-protocol-audit.md` 与 `docs/plans/2026-09-17-loop-self-improvement-proposal.md`
记录了协议层的设计意图——动 `crates/automed/src/protocol/` 之前读。

### 0.3 验证命令

**快检**（秒级，每个 batch 后必跑）：

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings      # CI 就是 -D warnings，本地别用宽松版
npm test --prefix apps/desktop                 # 127 个；纯 node --test，不需要 Electron 运行时
```

（`README.md` 的「Building」一节把这个数字写成 102，已经过时——2026-09-19 实测 127 pass / 0 fail。
数字以实跑为准，不要引用 README 的计数。`package.json` 里的 `npm test` 是 `node --test test/*.test.js`，
必须在 `apps/desktop` 下跑，所以用 `--prefix` 或先 `cd`。）

**全检**（分钟级，实施结束前跑）：

```sh
cargo test --workspace --all-targets
cargo test --test end_to_end -- --nocapture    # 真实 git worktree / rebase / merge，模型用替身
```

**一条命令跑完 CI 的全部门禁**（顺序与参数同 `.github/workflows/ci.yml`，热缓存约 35 秒）：

```sh
sh scripts/ci-local.sh
```

`.githooks/post-commit` 在每次提交后把它放到后台跑，结果落 `.git/ci-local/last.log`。
仓库没有 git remote，所以 workflow 从未真正执行过——本地这份就是唯一在跑的 CI。

**基线状态（2026-09-19 在 `simplify/dead-surface-and-three-bugs` @ 8d427af 实测，工作区干净）**
——跑之前先知道哪些本来就是红的，不要把既有失败当成自己改坏的：

| 命令 | 结果 |
|---|---|
| `cargo build --workspace` | 通过 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --all-targets -- -D warnings` | 通过 |
| `cargo test --workspace --all-targets` | 通过，260 domain + 510 automed + 29 e2e + 10 prompt + 7 real_cli |
| `npm test --prefix apps/desktop` | 通过，127 / 127 |

**基线现在是全绿的，这是新情况**：在此之前 fmt 红 147 处、clippy 红 38 处、
`cargo test` 有一个必然偶发红的挂钟断言（`git::tests::a_command_that_finishes_quickly_is_not_held_by_the_poll`，
已改成与阻塞式 `wait_with_output` 的相对基线对比）。所以任何红都是你改出来的，直接回滚或修正证明。

**计数一律以实跑输出为准。** README 的「Building」一节已经不再写测试数字——那四个数
（176/289/16/102）全部失真过，删掉计数比更新它更省事，别把它加回去。

**专项**：

```sh
# 免费、不需要账号。问每个 CLI 要自己的 --help，核对适配器表里每个 flag 都在。
# 机器上没装的 CLI 会跳过。CI 注释：would have caught the `-p` bug that made every session hang.
cargo test --test real_cli every_adapter_flag -- --nocapture

# 花钱，需要已登录的账号。
AUTOMED_REAL_CLI=1 cargo test --test real_cli -- --nocapture

# 整条循环，真实模型到真实 merge commit，约 40 分钟。
AUTOMED_REAL_CLI=1 AUTOMED_REAL_LOOP=1 \
  cargo test --test real_cli the_whole_loop -- --nocapture --test-threads=1

# 打包。CI 每次 push 都跑，所以改了 apps/desktop/package.json 的
# build.files / build.extraResources 之后必须跑一次。
sh scripts/package.sh
```

会 SKIP 的行：`needs_git!` 宏守着 git 与 launcher 相关用例，没有 git 的环境整段跳过；
`real_cli` 会跳过机器上没安装的 CLI。**报告里照实列出跳过了哪些**，不要把 SKIP 说成通过。

### 0.4 生产根与非生产根

**生产根**：

- `crates/autome-domain/src/**`
- `crates/automed/src/**`——包括 `protocol/seed/**`。那些 `.md` / `.toml` / `.yaml` / `.sh` 被
  `include_str!` 编进二进制，是运行时产物，不是资料。
- `apps/desktop/main.js`、`preload.js`、`src/**`、`renderer/**`
- `apps/desktop/package.json` 的 `build.files` 与 `build.extraResources`（打包清单，按 glob 生效）
- `.github/workflows/ci.yml`、`scripts/package.sh`、`rust-toolchain.toml`

**非生产根**：

- `crates/automed/tests/**`（`end_to_end.rs`、`real_cli.rs`、`prompt_rendering.rs`、`fixtures/`、`golden/`）
- `apps/desktop/test/**`
- `docs/**`、`legacy/**`
- `graphify-out/**`（未跟踪的分析产物）

**模糊地带**，单独判断：

- `scripts/gen-eval-seed.py` 不在运行时，但它的输出落在生产根里。改它等于改生产代码，改完要重跑。
- `crates/automed/src/protocol/seed/evals/**` 是生产（编进二进制）但内容是测试用例，且由脚本生成。

### 0.5 生成物、供应代码、迁移、锁文件

**绝不手改**：

- `crates/automed/src/protocol/evals.rs` —— 文件头原文：*"Generated by `scripts/gen-eval-seed.py`;
  edit the cases there, re-run it, and regenerate this file with the same script."*
- `crates/automed/src/protocol/seed/evals/**` —— 同一个脚本生成的整棵树。脚本 docstring：
  *"Re-run after editing; the output is committed."* 要改某个 case.yaml 或 grader，改脚本再重跑。
- 重新生成：`python3 scripts/gen-eval-seed.py`
- `Cargo.lock`、`apps/desktop/package-lock.json` —— 提交，不手改。

**两处显式长度是同步器，不是冗余**：

- `crates/automed/src/protocol/evals.rs`：`pub const SEED: [(&str, &str); 78]`
- `crates/automed/src/protocol/mod.rs`：`const SEED: [(&str, &str); 13]`

增删任何 seed 文件必须同时改这个数字，否则编译失败。这是有意的——`evals.rs` 文件头：
*"Listed explicitly rather than walked at build time so that a case which stops being embedded
fails here rather than on a user's machine."* 别把它改成 build 时遍历目录。

**镜像文档，在本仓库改没有意义**（下次镜像会覆盖）：`docs/development/requirements.md`、
`docs/development/technical-design.md`、`docs/design/*.html`。README 原文：*"mirrored from the
authoring repository"*。权威在 1.x 仓库的 `autome/docs/plans/`。

**不入库**（`.gitignore`）：`/target`、`node_modules`、`dist/`、`apps/desktop/core/`、
`apps/desktop/dist/`、`.autome-v2-local/`、`*.log`、`.DS_Store`。`graphify-out/` 当前未跟踪也未忽略。

**两个陷阱**：

- `apps/desktop/build/` 名字像产物，其实是源码目录：里面唯一被 git 跟踪的是手写的
  `entitlements.mac.plist`，签名构建必需（见 §0.6 第 9 条）。**不要因为目录名删它。**
- `apps/desktop/dist/` 里躺着一个 `automed.sqlite3`，是跑出来的残留，不在打包清单里。

### 0.6 受保护的契约与红线

删除下列任何一项都是产品决策，本技能只能报告并等用户拍板。

1. **JSON-RPC 方法名**——`crates/automed/src/dispatch.rs` 的 match 表（`"project.list" => …` 这类），
   加上 `dispatch_curation.rs`、`dispatch_protocol.rs`。渲染进程按字符串调用，Rust 侧静态搜索看不到调用者。
   删一个方法等于删一项产品能力。

2. **两份读 allowlist 的子集关系必须成立**：`pub const READ_METHODS: [&str; 18]`
   （`crates/automed/src/dispatch.rs`）↔ `ALLOWED_READ_METHODS`
   （`apps/desktop/src/ipc-gate.js:18`，**15 项**）。JS 那份是 Rust 那份的**真子集**，
   当前少三个：`env.detect`（渲染进程不需要）、`config.validate` 与 `events.since`
   （核心里有实现，但界面上没有任何东西调它们，2026-09-19 收掉了桥接）。
   **三个都不要「补齐」**——子集是对的，相等不是要求。
   `ipc-gate.js` 文件头原文：*"The read/write split is load-bearing: a method on this list must be
   side-effect free in the core (see `READ_METHODS` in crates/automed/src/dispatch.rs, which this
   mirrors). If the two ever disagree, a 'read' could mutate, and the separate write gate would be
   decoration."*
   `apps/desktop/test/ipc-gate.test.js:31` 与 `test/packaging.test.js:183` 按路径读 `dispatch.rs`
   做交叉校验；CI 还有一个专门的 `contract` job 断言该文件存在且含 `pub const READ_METHODS`。
   **移动这个文件或重命名这个常量会直接红 CI。**

   **写通道另有一份 allowlist，治理方式不同**：`ALLOWED_WRITE_OPS`
   （`apps/desktop/src/write-gate.js:22`，34 项）。它**不是**任何 Rust 常量的镜像，没有交叉校验测试，
   也没有 CI contract job——因为它列的是「渲染进程可以请求的 op」，不等于核心的方法名：
   `project.pick`、`open.path`、`open.terminal`、`config.set_theme` 由 Main 自己处理，核心里没有同名方法；
   而 `project.add` 被**故意排除**，文件注释原文：*"`project.add` is deliberately absent — it takes a
   path, and only Main may supply one (see `project.pick` below)."* 以及
   *"A renderer that could name a path could name `~/.ssh`."*
   在这张表里「补上 `project.add` 让两边对称」是把安全边界拆掉，不是简化。

3. **适配器 flag 表 ↔ 真实 CLI 的 `--help`**，由 `cargo test --test real_cli every_adapter_flag` 守。

4. **协议 seed 的路径字符串**（`protocol/mod.rs` 与 `protocol/evals.rs` 两张 SEED 表里的第一列）。
   那是用户 `~/.autome` 协议仓库的内容，已经在用户机器上落盘并被 git tag 版本化。改路径 = 改持久化格式。
   升级时提供新种子的那个标签名由 `upstream_tag_for()` 从 `seed().hash()` 派生
   （`protocol/mod.rs`，前缀 `protocol/upstream-`）。**不要把它改回手工维护的版本号常量**：
   原来的 `SEED_VERSION` 就是这么做的，两次改 seed 都忘了 bump，而 `offer_upstream_of` 先按标签名
   早退、后才做哈希比对，结果是升级后永远不再提供新种子。

5. **设计文档状态块的格式**（`crates/autome-domain/src/status_block.rs`）。README：任务进度存在仓库的
   设计文档里，*"It travels with the branch and survives a machine change."* 解析器放宽或收紧都会影响
   已经存在于各分支上的文档。

6. **SQLite schema 与回填**（`crates/automed/src/store.rs`、`backfill.rs`）——用户机器上有数据。

7. **SAME-MODEL 闸门**。README：*"Generation and evaluation never share a model. Review must differ
   from design, and audit from implementation. A configuration that violates this cannot be saved."*
   这条校验属于安全控制，见「安全边界」。

8. **ADR-0001**：*"No build, test, or release step may reference a path inside the 1.x repo."*
   不得为了复用把构建、测试或发布指向 `../autome`。

9. **`apps/desktop/build/entitlements.mac.plist` 里的 AppleEvents 能力**。README 明写：
   没有它，签名构建里会话启动器打不开终端，*"every session fails to start, in signed builds only"*
   ——本地未签名构建测不出来。

### 0.7 有意为之的重复与预留接缝

下面这些看着像熵，是设计。**未经用户明确要求不得提议删除或合并。**

- **`READ_METHODS` ↔ `ALLOWED_READ_METHODS` 的双份清单**：不要去重。两个进程、两种语言，
  共享一份会把安全边界降格成可绕过的配置。同步由测试保证（解析 `dispatch.rs`），而不是由共享代码保证。
- **`src/ipc-gate.js`（读）与 `src/write-gate.js`（写）结构相似**：有意分开。合并成一个带 mode 参数的门，
  会让「读不能改状态」这条不变式失去结构上的保证。
- **只有两个 crate，`dispatch.rs` 三千行**：这是 ADR-0001 的代价，不是熵。提议「按职责拆成四个 crate」
  等于违反决策。同一个 crate 内按组分文件是允许的做法——`dispatch_protocol.rs` 文件头：
  *"Kept out of `dispatch` because it is a self-contained group … and `dispatch` is already the
  longest file in the crate."*
- **`apps/desktop/src/*.js` 只有一个调用方**（main.js）：`ipc-gate.js` 文件头写明理由——
  *"Kept out of main.js so it can be unit-tested under plain `node --test` without an Electron
  runtime. main.js is the only caller and adds nothing this module does not already decide."*
  这是为可测性存在的一层，不要内联回 `main.js`。
- **模型替身与真实 CLI 两套测试路径**（`end_to_end.rs` 用替身，`real_cli.rs` 用真实二进制）：
  README 明写取舍——*"A stand-in cannot catch a flag that does not exist, or one that means
  something other than what you assumed — and that is the class of bug that bit this project hardest."*
  不要合并成一套。
- **`legacy/loop-plugin`**：1.x 的历史参照，不参与构建。删不删是产品决策，不是熵回收。
  （它在本仓库内，不违反 ADR-0001 的「不得引用 1.x 路径」——那条禁的是构建/测试/发布步骤指向外部仓库。）
- **`curation.rs` 的 `removal_experiments` / `Rule` / `RemovalExperiment` / `RETIREMENT_IDLE_TASKS`**：
  生产消费者确实是 0，2026-09-19 那轮有两个子代理各自独立把它报成死代码——**都是错的**。
  它是「规则移除实验」这个功能缺失的那一半：`rules.retire` 已经有 IPC 方法、SQLite 表、
  端到端测试与 UI 卡片，缺的是生成候选列表的那一层，而这几个类型正是为它写的。
  判断依据不是「谁在调它」，而是「`rules.retire` 的 `body` 参数从哪来」——今天无处可来，
  所以整条链路到不了。补全它需要新增规则命中追踪（`Rule.idle_tasks` 在 schema 里没有数据源）。
- **`Environment::runtime_ready` 与 `TaskMetrics::reopens_in`**：各 6 行、生产消费者 0、
  只被测试调用。删得掉，但删掉会让四个测出真实域规则的用例（「过期登录只挡自己那个运行时」
  「未知登录不等于过期」）失去表达方式，净减少不值这个代价。按 §5「太小、太不确定」降级保留。

### 0.8 动态入口

静态搜索找不到调用者但确实被用的路径，以及搜它们的方法：

- **JSON-RPC 方法名**：`grep -n '" =>' crates/automed/src/dispatch.rs`（55 条），另加
  `dispatch_curation.rs`、`dispatch_protocol.rs`。一个 Rust 函数「没人调用」通常只是因为它只从这张表进来。

- **从方法名找到 UI 消费者要走三跳，中间会改名**。渲染进程里**根本不出现** JSON-RPC 方法名，
  所以直接 `grep 'task.get' apps/desktop/renderer` 会得到 0 个命中，然后你会误判它没人用。
  真实链条：

  ```
  renderer/screens/*.js        read('getTask', taskId)          ← 驼峰桥接名
  preload.js:31                getTask: (taskId) => read('task.get')({ task_id: taskId })
  dispatch.rs                  "task.get" => task_get(...)
  ```

  `apps/desktop/preload.js` 是**唯一**的翻译点（49 条桥接：15 条 `read('…')` + 34 条 `write('…')`）。
  所以查一个方法有没有 UI 消费者，固定两步：

  ```sh
  grep -n "'task.get'" apps/desktop/preload.js          # 1. 拿到驼峰桥接名
  grep -rn "getTask" apps/desktop/renderer              # 2. 用桥接名再搜一次
  ```

  两步都空才是真的没有 UI 消费者——但还要排除 CLI 与测试（`crates/automed/tests/`）这两类消费者。

- **写通道的 op 名不等于方法名**：`write('project.pick')` 这类 op 由 Main 处理，核心里没有同名方法；
  反过来核心有的写方法不一定在 `ALLOWED_WRITE_OPS` 里（`project.add` 就是故意不在，见 §0.6 第 2 条）。
  判断写路径的可达性时三张表都要看：`dispatch.rs` 的 match、`write-gate.js` 的 `ALLOWED_WRITE_OPS`、
  `preload.js` 的桥接表。
- **`include_str!` 的 seed 路径**：`grep -rn 'include_str!' crates/automed/src`。
  `protocol/seed/` 下的文件「没人引用」是假象。
- **协议 prompt 与 eval 里的名字**：角色名、节点名、状态块标记同时出现在 Rust 枚举和
  `seed/prompts/*.md`、`seed/evals/**` 里，模型按名字用它们。删任何一个枚举变体前先跑
  `grep -rn '<名字>' crates/automed/src/protocol/seed`。
- **退出标记与 wrapper 脚本协议**（`crates/automed/src/launcher.rs`）：另一端是 shell，纯字符串协议。
- **CI 按名字跑的测试**：`cargo test --test end_to_end`、`--test real_cli every_adapter_flag`。
  重命名测试函数会让 CI 那一步静默地变成「跑了 0 个用例」而不是失败。
- **electron-builder 打包清单**（`apps/desktop/package.json` 的 `build.files`、`extraResources`）：
  按路径 glob 生效，新增目录不在清单里就不会进 `.app`——只有 `sh scripts/package.sh` 能发现。
- **根配置**：`rust-toolchain.toml`。
- **`legacy/loop-plugin/.claude-plugin/plugin.json`**：插件入口（不参与本仓库构建）。

### 0.9 审计记录落点

默认只在对话里给结论，不落盘。

用户要求落盘时：写到 `docs/plans/`，命名 `YYYY-MM-DD-<主题>.md`，与已有的
`2026-09-16-loop-v1-protocol-audit.md` 一致。中文。

引用规则：每条断言带 `path:line`；跑过的命令附实际输出摘要；没能核实的写「未能核实」而不是省略；
SKIP 掉的测试照实列出。

## 1. 选择范围与模式

范围按下面顺序解析，取第一个命中的：

1. 用户给了路径、目录、包名、crate 名 → 只看那个范围，读完整文件而不是 diff。
2. 仓库有未提交改动或分支相对主干有提交 → diff 范围：`git diff HEAD` 加 `git diff <主干>...HEAD`。
3. 都没有 → 整仓审计。

模式看用户措辞：「找 / 审 / 列 / 看看有没有 / audit / find / review」→ **audit**，只报告不改；
「简化 / 清理 / 删 / 收一下 / apply / simplify / clean up / remove」→ **apply**，实施并验证。
diff 范围默认 apply，整仓范围默认 audit。删除可达能力、公开 API、持久化格式、兼容路径属于产品决策：
无论什么模式都先把取舍摆出来，等用户拍板。

## 2. 建立契约

1. 读 §0.2 列出的文档，读 §0 全部。
2. `git status`。用户已有的未提交改动不是你的；分清哪些是你要审的、哪些要绕开。
3. 顺着真实运行路径走一遍：入口、配置装载、依赖注入、事件与队列、持久化、后台任务、线协议。
   这一步决定你知不知道「谁在消费」。
4. apply 模式：跑 §0.3 的快检建立基线。基线本来就红的，记下来，不要事后归咎于自己的改动。

## 3. 找候选

**diff 范围**用五个角度，每个角度独立过一遍 diff（有 Agent 工具就并行派五个子代理，每个只拿一个角度；
没有就自己按顺序做，并在报告里说明是单次串行审查）。角度定义见
[references/review-angles.md](references/review-angles.md)：复用、化简、效率、高度、约定。

**路径或整仓范围**用九类熵逐类扫，定义、搜索方法与反例见
[references/entropy-taxonomy.md](references/entropy-taxonomy.md)：未消费的面、镜像事实、投机泛化、
多余的路由或层、生命周期重复、错位的防御、手搓基础设施、支持性残留、加了又弃的残留。
按域并行派子代理时，每个子代理必须带证据回来，从生产代码最大的增量开始，而不是停在显而易见的未用符号。

每个候选记：`file`、`line`、一句话 `summary`、具体代价（重复了什么、浪费了什么、多维护了什么）。

## 4. 证明或否决

每个候选按 [references/proof-protocol.md](references/proof-protocol.md) 走完再决定：
穷尽搜索（符号、字符串、路径、配置键、wire 名）→ 按 §0.4 把命中分成生产 / 非生产 / 模糊 →
读调用方与被调用方，含 §0.8 的动态入口 → 看历史（`git log -S`、决策记录）→ 有状态或异步的画所有权图 →
说清删掉后失去什么 → 估净减少 → 找出「删错了会最先失败的最小检查」。

证据记录固定五行：

```
[confidence: high|medium|low / risk: low|medium|high] <候选一句话>
evidence:  <生产消费者 0 个；非生产 N 个在 …；历史 …>
cut:       <具体删什么，到哪一层>
tradeoff:  <失去的能力或兼容性；没有就写「无可见行为变化」>
verify:    <删错了会最先失败的最小检查>
```

**保留或降级**：有生产消费者；动态可达性排除不了；决策记录仍然成立；改动只是把复杂度挪个地方；
候选太小、太不确定或超出范围（改成一条带标签的 TODO 更合适）。§0.7 列出的东西不提议删。

## 5. 实施（apply 模式）

- 一次只动一个所有权边界；一个 batch 可回滚。
- 契约删到底：导出、类型、构建入口、注册项、配套测试、文档引用一起走，不留半截。
- 幸存行为的测试保留；只删专属于被删契约的测试。
- 镜像状态收敛到一个表示，不是删掉其中一个副本后再加同步代码。
- 优先级：删除 > 标准库 > 已装依赖 > 新依赖。
- 不加没有义务的 shim、别名、兼容层。
- 净减行数是证据不是目标；可读的显式代码优先于紧凑代码。

## 6. 验证

1. 重新搜索被删的名字，确认没有残留引用（含字符串与配置）。
2. 先跑 §4 记的最小检查，再跑 §0.3 快检，再跑全检；专项按前置条件决定跑不跑，不跑要写明。
3. 重跑当初产生候选的分析器（lint、未用导出扫描），确认候选消失而不是被静音。
4. `git diff --check`；完整读一遍 diff。
5. 对照 §0.6：公开面、持久化格式、线协议的可见行为有没有变。
6. 失败时：回滚这个 batch 或修正证明。**不得放宽断言、削弱类型、跳过测试来让检查通过。**

## 7. 报告

**audit**：候选按 置信度 / 风险 / 净减少 排序，每条用五行证据记录；单独列「有价值但故意保留」的候选与原因；
说明扫过的域、没扫的域、跑过的检查。
**apply**：删掉的契约；可度量的减少（文件 / 导出 / 依赖 / 状态 / 概念）；可见行为变化（没有就说没有）；
实际执行的验证命令与结果，SKIP 的行；有意跳过的候选与原因。
没有 Agent 工具时说明是单次串行审查。落盘规则见 §0.9。

## 安全边界

不管证据多充分，都不简化掉：信任边界上的校验与鉴权、安全控制、无障碍基线、防数据丢失的检查、
持久数据的兼容路径、把资源带到静默态的清理逻辑（rollback、回调隔离、终态仲裁、worker 所有权、dispose）。
这不是性能审计；除非用户点名，不因为「更快」而改。

本仓库追加的红线，每条带出处：

- **SAME-MODEL 闸门**（`README.md`：*"Generation and evaluation never share a model … A configuration
  that violates this cannot be saved."*）——这是安全控制，不是可放宽的校验。
- **读写双通道与两份 allowlist**（`apps/desktop/src/ipc-gate.js` 文件头、`src/write-gate.js`）——
  信任边界校验。不得合并、不得放宽、不得去重（见 §0.7）。
- **渲染进程永远不能指定文件系统路径**（`README.md`「Layout」：*"the renderer can never name a
  filesystem path"*）——任何让渲染进程传路径的「简化」都不做。
- **两个停顿点与不推送**（`README.md`：*"Autome never pushes, never opens a pull request, and never
  merges without you pressing the button."*）——产品承诺，不是可优化掉的流程环节。
- **签名是 opt-in 而不是 best-effort**（`README.md`：*"an unsigned build that claims to be signed is
  worse than one that says it is not"*）——不要为了少一个分支让签名路径默认打开。
- **ADR-0001 的 provenance 三条**（`docs/adr/0001-greenfield-independent-repo.md`）——
  尤其是「不得让构建、测试、发布步骤引用 1.x 仓库路径」。
