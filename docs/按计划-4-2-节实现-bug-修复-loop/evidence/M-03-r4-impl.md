# M-03 门禁修复证据 · 实现 #4

本轮 4/30，起点 fd39e0e（Autome 在 8683af4 上报告 Test 失败）。只修复已有 M-03 的门禁，不推进 M-04，M-03 保持待审。上一审计为 M-02 通过，没有要求移植的临时缺陷检查；本次失败检查本身已在项目测试体系。

## 根因与修复

`task_card_reply_rejects_the_design_with_the_text_verbatim` 遍历 feature/bug，均用需求文档替身并断言 await_design_approval。M-03 已将新 Bug 接入 Locate，因此 Bug 分支在 reply 前就失败，实际 node 为 Null。设计 §11 第 5 条明确废止这个 K1 过渡断言，却把调整排到 M-04，造成跨里程碑门禁断裂。

按 §11 第 5 条把输入集合改为 feature，保留全部需求路径断言，包括缺参退出 2、中文/双空格/换行原文传递与回到批准设计。产品代码未改。设计就地明确这项调整与 M-03 配套，新 Bug reply 的实现和替代用例仍属开放的 M-04，未声称已覆盖。

同类检查：搜索 CLI 测试中的 feature/bug 共同循环及 await_design_approval，只有此处错误地把新 Bug 当作需求；另一处属于需求主流程。设计 §11 第 3、4 条的卡片测试已在 M-03 更新。搜索结果在 `.autome/output/r4-related.txt`。

## 红绿复现与命令

- 原样运行 `sh scripts/ci-local.sh`：退出 1；Test 中 CLI 集成 7 通过、1 失败，失败为上述用例；另有沙箱 Web test 环境失败。日志 `.autome/output/r4-ci-red.txt`。
- 原样运行 `cargo test -p autome-cli --test cli task_card_reply_rejects_the_design_with_the_text_verbatim`：退出 101，1 个用例失败，断言 Null != await_design_approval。日志 `.autome/output/r4-reply-red.txt`。
- 调整后运行相同专项命令：退出 0，1 个用例通过；日志 `.autome/output/r4-reply-green.txt`。

## 自审清单（本轮只修复门禁）

1. **情形表逐行**：设计 §11 第 5 条的 feature 分支保留且实跑通过；K1 新建 bug 走需求 Loop 的旧情形按 A1 废止。新 Bug reply 的替代情形尚待 M-04，未标完成。§11 第 3、4 条卡片情形在全量中复验。
2. **失败分支**：原用例缺少补充信息参数时退出 2 的检查保留并通过；原 Bug 分支在进入 reply 之前就失败，不能作为 Bug reply 的有效覆盖。授权拒绝、读盘失败、超时、外部服务不可达不适用于本轮输入集合调整，未改对应实现或绕过检查。
3. **全部状态迁移**：feature 创建至 await_design_approval，再 reply 回需求循环、再次 await_design_approval 的原流程通过。没有新增产品状态迁移；Bug 状态迁移由全量已有领域与 e2e 检查复验，新 reply 运行时路径留 M-04。
4. **输入域边界**：缺参数、中文、多字节、双空格、换行的既有断言全部保留并通过。日期、整数最大值、前导零等不适用于本轮测试 kind 集合调整。

## 数量与限制

本轮新增测试函数 0、删除测试函数 0；CLI 集成用例仍为 8，相比上一轮差 0。但必须明确：一个测试内部输入情形从 2 减为 1，移除的是任务明令废止的新 Bug 走需求 Loop 情形，不是已实现的新 Bug reply 场景。M-04 须补设计点名的 Bug reply 用例。

上一实现轮 M-03 未跑 workspace 全量，仅报告去重 1011 个专项用例；不能把它直接当全量基线。最近已验证的 M-02 workspace 基线为 1047，本轮预期 1058（M-03 新增 8 个 e2e + 3 个调度器单测，本轮 +0）。Web 基线 175，本轮仍 175；环境失败与跳过不得算通过。

修复后的 `cargo test --workspace --all-targets` 在同一门禁脚本内通过：1058 = CLI 单元 17 + CLI 集成 8 + 领域 345 + automed 单元 598 + e2e 67 + prompt 16 + real_cli 7，0 失败、0 忽略。相比最近完整基线 1047 增加 11，均来自上一轮 M-03；本轮差 0。单独重复运行的 e2e 与 Adapter flags 不重复累计；real_cli 的环境条件不等于真实模型验收。

最终原命令 `sh scripts/ci-local.sh` 退出 1（154 秒），日志 `.autome/output/r4-ci-green.txt`：Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 通过，只有 Web test 失败。Web 共 175（相对用户给的基线 175 差 0），156 通过、10 失败、9 跳过；失败原因是监听 127.0.0.1 被沙箱拒绝 `listen EPERM`，同时有 `EMFILE` 文件监听错误。Desktop 共 104，103 通过、1 跳过（Electron 二进制不可用）。没有修改 apps/、scripts/ 或测试门禁来规避环境限制；不能声称本地全量门禁通过。

`git diff --check` 通过。M-03 保持待审，implementation-round 为 4/30；待 Autome 在提交后宿主环境重跑门禁。本轮提交使用仓库已有 `NO_CI=1` 跳过重复的后台 post-commit CI（前台已执行相同脚本，Autome 仍会独立运行宿主门禁），不启动下一会话。
