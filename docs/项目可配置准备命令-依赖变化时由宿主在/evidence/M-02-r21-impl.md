# 实现 #21：指定冲突文件复验

## 范围与修复说明

起始 HEAD：b04728d。工作树干净，`git ls-files -u` 无输出，rebase-merge / rebase-apply 均不存在，render.rs 无冲突标记。指定冲突已由 e329a0a 修复，当前同时保留 setup 事件和单仓、工作区 rebase 文案，本轮不重复修改产品代码。

简报与审计 #18 一致：M-01..M-04 已由审计关闭，无开放或待审里程碑，也没有退回缺陷需要逐字搬入测试或红绿复现。保留里程碑表与已完成状态，仅更新 implementation-round 为 21/20、M-02 最新证据指针；next-action 保持无。不新增里程碑、不自行启动下一会话。

## 命令与结果

- `cargo test -p autome-cli > .autome/output/r21-cli.txt 2>&1`：退出码 0，22 + 11 = **33 个用例**全部通过，0 失败、0 忽略、0 过滤。相比实现 #20 同范围基线 33，差 **0**，没有增删测试。
- `cargo fmt --all -- --check > .autome/output/r21-fmt.txt 2>&1`：退出码 0。
- `git ls-files -u` 为空；render.rs 冲突标记扫描无匹配；通过 `git rev-parse --git-path` 检查两个 rebase 状态目录均不存在。
- `git diff --exit-code 2553923 -- .autome/config.toml`：退出码 0。
- 本轮未重跑 automed lib、e2e、全工作区、Clippy、Desktop、Web 或宿主门禁；历史审计的 1124 个用例不冒充本轮全量结果。H-01/H-02 仍待合并前人工确认。

## 标待审之前的自审清单

本轮没有开放里程碑，不执行标待审；针对指定冲突的渲染范围检查如下，涉及测试均随 CLI 全量实际运行：

1. **情形表逐行**：设计 §8 started 在 worktree、verify、main 三种检出逐一验证标签、命令、日志；result 在三种检出逐一验证成功/失败、退出码、日志；skipped 验证工作区跳过原因。`setup_events_are_printed_by_autome_run` 通过。
2. **失败分支**：同一测试逐检出验证退出码 7 的失败文本与日志。授权拒绝、读盘失败、超时、外部服务不可达不适用本轮纯渲染复验，执行层未改动。
3. **全部迁移**：状态迁移不适用，未改状态机；事件通道覆盖 setup 三种事件、merge.auto 的 active=true/false、merge.rebased 单仓与工作区，四个相关渲染测试通过。
4. **域边界**：rebase 测试覆盖空数组、单成员、多成员、空仓库名、长短 SHA。日期与数值极值不适用本轮范围。

## 提交

提交本轮证据、设计轮次与证据指针、一行 retro；保持当前分支，不推送。
