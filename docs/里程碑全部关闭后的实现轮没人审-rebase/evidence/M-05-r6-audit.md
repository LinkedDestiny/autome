# M-05 r6 审计证据 · 审计 #5

结论：**通过**（已完成，reopen 0）。

## 验收命令（自己跑）
- `cargo run -p automed -- protocol eval protocol/` 退出码 0：8 项通过、0 不通过、1 警告（41 行无 CHANGELOG 说明的条文，既有历史警告）。
- `prompt_rendering`：35 通过，等于 M-03 结束时的数量，无差。
- `grep -c '^## protocol/v' protocol/CHANGELOG.md` = 7（v1..v7 各一）；`^## 集成复验$` 命中 loop-protocol.md:310；technical-design.md 含「集成复验」2 处（提交范围、集成复验两段，位于 §6/§8 相关小节）。

## 负向对照（均已还原，`git status` 干净）
1. 把「集成复验」改成 `#### 集成复验` 并挪进 `### 审计轮` 内：`^## 集成复验$` 计数 0，验收命令失败。但 `prompt_rendering` 仍是 35 通过——金样用空协议文本，契约原写「两个金样失败」不成立。属契约描述不准（验证口径：真正拦它的是 grep），产品无错；已把契约负向对照改成真实行为，不计 reopen。
2. CHANGELOG v7 一条 `metric` 改为 `bogus_metric_xx`：静态门报「第 266 行 metric 不在词表」，1 项不通过。
3. 去掉 v6 标题（改名）：计数 6 ≠ 7。

## 独立检查
- `section("实现循环")` 的取法（下一个同级或更高标题截止）：`## 集成复验` 是同级兄弟，实现循环 208–309 行内「集成复验」0 次 → 审计简报里 `实现循环` 摘录不会带出新正文，符合“不流进普通审计简报”。
- 两份协议的 `kernel-contract` 区与主线 merge-base 逐字相同（md5 一致）。
- session-protocol 的精简（J-3）只删解释性文字，规则含义（不启动下一会话、分段读文件、协议失败不含提交失败/缺证据/实现缺陷/工具链不符）保留。
- 「Autome 会检查什么」新增两行与 M-02 已实现并验收的守卫（集成复验无证据、`impl_closed_milestone`）对应。
- CHANGELOG J-1 behavioral 带 `eval: evals/audit-integration-review/`（目录存在，静态门通过）；J-2/J-3 clarify。

## 未跑
全量门禁、`ci-local.sh` 由宿主合并前运行。宿主快检（fmt/clippy/web lint/typecheck）在 ad9b47adf2 上退出码 0。无人工验收项。
