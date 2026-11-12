# 审计 prompt 与会话范围守卫冲突

status: 已定位
fix-round: 1/3
复现命令: cargo test --offline -p automed --test prompt_rendering scope_contract_
根因: 审计模板允许未限定落点的加强验收，审计、复盘和复核模板未告知文档目录写入边界，而调度器按会话提交差异强制执行该边界。
复核: 待复核

## 现象

定位基线：`e0b0bb2b0f379377eb0e51e8dfc16e1624e41d7a`。
已完整读取任务卡；本轮未发现 `审计-prompt-replies.md`。

审计 prompt 要求「当场加强验收或补检查」，未限定只能修改本任务文档或使用不入库的临时检查。审计方据此提交仓库测试、格式化或 lint 修复时，范围守卫会返回协议失败。复盘和复核虽分别已有「不改代码」「不要代替修复轮修代码」，仍未声明完整目录边界和越界后果。

任务卡列出的 T-47 和历史提交是用户提供的事故线索，本轮没有重演这些历史会话；以下证据来自当前代码和真实 prompt 渲染测试。

## 复现

本任务是单 Git 仓库，根目录的 Cargo workspace 包含 Rust crate，不是 Autome 多仓库工作区，因此无需 `repos:` 或切换成员仓库。宿主在当前 worktree 根目录执行状态块中的命令即可；无需网络、模型调用或时序等待。

新增 `crates/automed/tests/prompt_rendering.rs` 中四个 `scope_contract_` 测试，调用实际协议种子和 `build_prompt`：

1. 审计、复盘、复核各自断言渲染结果明确声明只能修改 `{doc_dir}/` 下文件，并告知被跟踪文件、产品代码、测试、格式化、lint 和协议失败。
2. 审计「验证缺口」条款必须在操作指令自身限定验收契约、`.autome/output/` 不入库检查及实现轮归属，不能仅在其它段落提及这些概念。

连续两次运行复现命令均退出 **101**：`0 passed; 4 failed`。三个范围告知测试报「缺少本任务文档目录的明确写入边界」，验证缺口测试报「验证缺口的操作范围缺少：验收契约」。这是断言失败，不是编译、依赖、超时或命令缺失。

对照命令 `cargo test --offline -p automed --test prompt_rendering -- --skip scope_contract_` 退出 **0**，原有 16 个测试全部通过，证明现有 golden 会接受这份有缺口的模板。

完整原始输出留在 `.autome/output/scope-regression.log`、`.autome/output/scope-regression-repeat.log` 和 `.autome/output/prompt-baseline.log`，不提交临时输出。该回归锁定 prompt 契约冲突，不声称能确定性重现模型自行修代码的选择。

## 根因分析

- `protocol/prompts/audit.md` 的「验证缺口」分支未限制加强验收的文件落点；三种目标模板均缺少任务卡要求的完整范围声明。审计和复盘末尾还声称「协议失败」只用于协议格式问题，与实际范围失败分支不一致。
- `crates/automed/src/launcher.rs` 的 `role_prompt` / `bug_prompt` 只做模板选择和占位符替换，没有补入范围规则。因此是模板契约遗漏，不是渲染丢字。
- `crates/automed/src/scheduler.rs` 的 `guard_session_scope` 对 Audit / Retro 比较 `session.started.scope_heads` 到 `HEAD` 的 `git diff --name-only --no-renames`，任一越界路径都会拒绝。`BugStep::FixReview` 在 `crates/autome-domain/src/session.rs` 中映射为 Audit，故复核也受限。守卫不区分纯移动、测试增强或行为改变；这符合任务卡要求保留的独立审计边界。
- 范围失败进入协议失败处理；现有错误只列出允许目录和越界文件，未给出会话开始提交及 rerun 前的恢复步骤。
- 原有 golden 测试校验渲染文本一致性，不能发现提示内容与运行时守卫不一致；新增回归补上这一缺口。

后续修复应保留守卫，让三份模板明确边界；限定审计加强验收的落点，项目测试（含格式、lint）失败交回实现轮处理；补齐恢复提示。协议变更按 `protocol/README.md` 更新版本、行为 eval 与 golden。定位轮不修改协议种子、产品代码或 golden，也不执行付费行为 eval。

## 修复说明

第 1/3 轮修复：

- 三份模板明确文档目录边界及越界后果；审计验证缺口仅可补验收契约或不入库临时检查，仓库测试、格式与 lint 失败按实现缺陷退回，禁止审计方自行修复。
- 保留范围守卫判定，仅在错误中加入每个仓库的会话开始 SHA、恢复越界文件并提交后再 rerun 的步骤；补充恢复提示断言。
- 协议升级为 v3，新增行为 eval 并编入种子，更新审计/复核 golden 和版本测试夹具。定位轮的四项回归测试文件未改动。
- 原复现命令修复前 4 failed，修复后 4 passed；全部 prompt 渲染 20 passed；协议相关测试 61 passed；范围守卫测试 2 passed。
- 行为 eval 脚手架已实跑；付费模型行为 eval 未运行，不声称行为层改前红、改后绿。
- 协议静态门 8 passed、0 failed、1 条历史条文覆盖警告；格式检查和 Clippy 通过。完整 `sh scripts/ci-local.sh` 未通过：Desktop 的 Electron DOM harness 收到 SIGABRT，Web 类型检查在已有 `scripts/notifier-live-regression.mjs` 报隐式 any，Web 实测受本机监听 EPERM 和文件监听 EMFILE 阻断。未改动这些无关文件或弱化测试。前次运行还包含中间版本的协议失败，已由最终协议测试复验通过；最终 Rust 全量运行中 automed 单元测试 650 passed，端到端 111 passed、1 failed（另一处旧 v2 版本断言）；将该版本断言同步为 v3 后，单独重跑该用例 1 passed，其余 111 项未重复运行。real_cli 7 passed；原有忽略项未改动。完整 CI 因上述 Desktop/Web 失败仍未通过，不能作为发布通过证明。原始日志均留在 `.autome/output/fix-1-*.log`，不提交。

## 复核意见

待独立复核。
