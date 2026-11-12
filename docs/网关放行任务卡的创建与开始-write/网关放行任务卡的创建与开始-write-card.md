# 网关放行任务卡的创建与开始（W6/W7/W9 的前置）

## 背景
计划 §3 定了任务卡的三个来源，网页来源（W6「新建任务卡」）要经 `apps/web/server/gates/write-gate.ts` 把 `task.create` 交给内核。内核（`crates/automed/src/dispatch.rs`）已经支持 `task.create` 的 `kind`/`brief`/`draft`、`task.start`、`task.reply`，以及读方法 `project.probe`；网页端的两份白名单还没跟上：
- `write-gate.ts`：`ALLOWED_WRITE_OPS` 没有 `task.start`、`task.reply`；`task.create` 的散文字段只有 `request`（8000），`kind`/`brief`/`draft` 不被识别，普通字符串上限 512，整体载荷上限 `MAX_PAYLOAD_JSON_LENGTH = 32KB`，装不下 §3.1 说的 64KB 正文。
- `read-gate.ts`：`ALLOWED_READ_METHODS` 13 项，比内核 `READ_METHODS`（17 项）少 `project.probe` 等；现有测试只检查「网页端 ⊆ 内核」，不检查漏项。
W6 的「开始」按钮、W7 的「补充信息」、W9 的「确认前先 probe」都会被网关拒绝。

## 要做什么
1. `write-gate.ts`：放行 `task.start {task_id}`、`task.reply {task_id, info}`；`task.create` 识别 `kind`（feature|bug）、`draft`（bool）、`brief`（散文，上限 64KB，按 UTF-8 字节计）；`task.create` 的载荷上限提高到能装下 64KB brief + 其余字段；`info` 作为散文字段给合理上限（与内核一致）。
2. `read-gate.ts`：补齐内核 `READ_METHODS` 里缺的读方法（至少 `project.probe`）。
3. 测试：`test/write-gate.test.ts`、`test/read-gate.test.ts` 补正反例；读白名单测试加「内核有而网页端没有」的反向检查，列出内核 READ_METHODS 与网页端的差集并断言为空（或显式列出有意不放行的方法）。

## 不做什么
不碰 `crates/`；不做 W6 的表单与页面；不改桌面端的白名单副本（W11 删除）。

## 验收标准
1. 通过网关发 `task.create {kind:"bug", brief:<60KB 文本>, draft:true}` 能到达内核并创建 draft 任务；超过 64KB 被拒并给中文原因。
2. `task.start`、`task.reply` 经网关可用；参数缺失或类型不对被拒。
3. `project.probe` 经网关可读。
4. 反向检查测试在内核新增读方法而网页端未放行时会变红。
5. `npm test --prefix apps/web` 全绿，用例数相对基线只增不减。

## 参考
`apps/web/server/gates/write-gate.ts`、`apps/web/server/gates/read-gate.ts`、`crates/automed/src/dispatch.rs`（`READ_METHODS`、`task.create` 参数）、计划 §3.1、§3.3、§5。
