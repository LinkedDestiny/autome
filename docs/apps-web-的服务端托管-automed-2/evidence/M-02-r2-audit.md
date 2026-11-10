# M-02 · 审计 #2 复验

## 验收命令
`cargo build -p automed`、`npm run lint`、`npm run typecheck`、`npm test`（apps/web）、`npm test`（apps/desktop）：
lint、typecheck 无错误；Web 113 用例全过（10 文件，契约 67+46=113，skipped 0）；desktop 104 用例，103 通过、0 失败、1 跳过（electron dom harness，缺 electron 二进制，基线同）。

## 负向对照（各自注入后跑 read-gate、write-gate 两个测试文件，再 git checkout 还原）
| 注入 | 变红的用例 |
|---|---|
| 写白名单加 `project.add` | project.add is not reachable / workspace answer carries a choice / 取任意 path 的方法不可达（3 个） |
| 放开非空 `doc_refs` | doc_refs 非空一律拒绝 / array allowed only where table says（2 个） |
| 附件校验放宽到任意字符串 | 附件必须是上传编号 / array 用例（2 个） |
| 读白名单加 `project.add` | every allowed method is a read in the core / write method refused on read channel（2 个） |
| 读门禁 import 写门禁 | 两份门禁互不 import（1 个） |
还原后 `git status` 无源码改动。

## 自造检查
对 `validateWriteRequest` 探测：小写 UUID 通过；大写 UUID、UUID 后接 `/../x`、`/etc/passwd` 作附件均拒绝；`doc_refs` 为 `[]` 通过，`null`、字符串、`['a.md']` 拒绝；`project.add`、`project.probe` 为未知 op。额外键（如 `task.create.path`）放行，与 desktop 原行为一致，内核 `task_create` 不读该键；`open.path` 带 path 放行，但它是网关本地 op（M-05 前回 501），不到内核。均不属缺陷。
