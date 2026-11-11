# 实现 #20：指定冲突文件复验

## 范围与修复说明

起始 HEAD：b9c4d65。工作树干净，索引无未合并条目，rebase-merge / rebase-apply 均不存在，`crates/autome-cli/src/render.rs` 无冲突标记。该文件最近修改为 e329a0a（fix(cli): preserve rebase rendering alongside setup events）；当前实现同时保留 setup 三种事件与单仓、工作区 rebase 文案，指定冲突已解决，本轮无需重复修改产品代码。

简报、设计、审计结论及审计 #17 证据一致：M-01..M-04 均已由审计关闭，没有开放或待审里程碑，没有退回缺陷需要逐字搬入回归用例或红绿复现。本轮仅复验指定冲突所属 M-02，保留里程碑表与已完成状态，不新增关闭标记、不虚构待审项。implementation-round 写 20/20，next-action 保持无。

## 命令与结果

- `cargo test -p autome-cli > .autome/output/m02-r20-cli.txt 2>&1`：退出码 0，22 + 11 = **33 个用例**通过，0 失败、0 忽略、0 过滤。与实现 #19 同范围基线 33 相比差 **0**，未增删用例。
- `cargo fmt --all -- --check > .autome/output/m02-r20-fmt.txt 2>&1`：退出码 0。
- `git ls-files -u`：空；冲突标记扫描：无匹配；使用 `git rev-parse --git-path` 定位的两个 rebase 状态目录均不存在。首次探测误用了 zsh 特殊变量 `path`，导致该次后续 git 调用不可用；换用 `rebase_state_path` 后完整重跑成功，以重跑结果为准。
- `git diff --exit-code 2553923 -- .autome/config.toml`：退出码 0，配置未修改。
- 本轮未重跑 automed lib、e2e、全工作区、Clippy、Desktop、Web 或宿主门禁；不将历史全量结果视为本轮证据。H-01/H-02 仍需用户合并前确认。

## 标待审之前的自审清单

没有开放里程碑，本轮不执行标待审；针对冲突涉及的渲染范围逐项检查，以下既有用例均随本轮 CLI 全量实跑：

1. **情形表逐行**：设计 §8 的 started 对 worktree、verify、main 分别检查检出标签、命令和日志；result 对三个检出各检查成功与失败、退出码和日志；skipped 检查工作区跳过原因。`setup_events_are_printed_by_autome_run` 通过。
2. **失败分支**：上述用例逐检出验证退出码 7 的失败文案与日志。授权拒绝、读盘失败、超时、外部服务不可达不适用本轮纯渲染复验，执行层未修改。
3. **全部迁移**：状态迁移不适用，未修改状态机；事件通道覆盖 setup 三种事件、merge.auto active=true/false、merge.rebased 单仓及工作区，四个相关渲染用例通过。
4. **域边界**：rebase 用例覆盖空数组、单成员、多成员、空仓库名、长短 SHA。日期与数值极值不适用本轮渲染范围。

## 提交

提交本轮证据、设计状态轮次与最新证据指针、一行 retro；不切换分支、不推送、不启动下一会话。
