# M-01 · 实现 #1 · 审计独立复验

结论：通过。

## 负向对照（先做，后还原；`git status` 干净，复验 103/103）
- 把 `### 5.6` 改成 `### 5.3.1` → `M-01 FAIL 100/103`（多出的标题行、5.6 位置、缺 5.6 标题）。
- 全文 `127` 换成 `12x` → `FAIL 102/103`（缺 127）。
- 在 §5.1 已有行末尾加一个字 → `FAIL 102/103`（「删除或改写了 1 行」）。
- 第一次尝试用 BSD sed 不支持的 `0,/re/` 地址，改动没有生效，检查仍 PASS；换成 `10s/$/ x/` 才触发失败。说明验收命令对改写已有行有鉴别力，不是对任何输入都过。

## 验收命令
`... sh -s M-01` → `M-01 PASS 103/103`，退出码 0。

## 验收 2 逐项核对（对 `bug_loop.rs` / `task.rs` / `scheduler.rs` / `gate.rs`）
- §5.1 追加段：Draft 不占槽位、Start 仅 Draft 接受、两条拒绝信息原样存在于 task.rs；5 个出处测试名均存在。
- §5.6 适用范围：拒绝信息「Bug 任务需要独立复核…」存在于 dispatch.rs 与 bug_loop.rs；`uses_bug_loop`、旧 bug 任务测试存在。
- 节点表：7 个线上名与 `render.rs` 的 `node_label` 标签一致。
- 转换表：locate 三种 status 去向、fix→FixGate 恒定、FixGate 通过→FixReview 会话、复核通过→Rebase、打回→fix（FixRejected）、Rebase 冲突→fix、MergeGate 失败→fix，与 `apply_for` 匹配臂一致；出处引用的 16 个测试名全部存在。
- 复现退出码：成功复现 = 未超时 ∧ 非 0 ∧ 非 126/127；why 为 timed_out / not_runnable / exited_zero；`TIMED_OUT = 124`、默认超时 30 分钟，均与代码一致。
- 连续两次：`repro_misses >= 1` 即进 await_info，第一次回 locate 并注入命令、退出码、原因、日志尾，与文字一致。
- 全部出处路径存在（验收脚本已查）。

## 门禁
`sh scripts/ci-local.sh` 全部通过（423s，退出码 0）。仅文档改动，符合设计的「不改变任何测试结果」。

## 缺陷与缺口
无产品缺陷，无验证缺口。
