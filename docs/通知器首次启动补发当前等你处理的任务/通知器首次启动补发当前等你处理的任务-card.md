# 通知器：首次启动补发当前「等你处理」的任务；最近已通知状态落库，重启不重复，离开再进入要重新通知
## 背景
W4（T-16）的设计明确选择了两件事：首次启动从 `latest_seq` 起步不补历史（设计 §188-190，`apps/web/server/notifier.ts:22-27`），最近状态只存进程内 `Map` 不持久化（设计 §166-169，B-03 当时裁定忽略）。T-38 定位轮核实后判「不是 Bug」——行为符合既有设计。本任务是**有意改变这两条设计**：装好面板的第一天，已经停在 `await_merge` / `await_design_approval` / `await_info` / `not_a_bug` / 失败 的任务必须出现在铃铛里；面板重启不应把同一次等待再通知一遍。
## 要做什么
1. 首次启动（无游标）：先经 `dashboard.get`（或 `project.get` 遍历）拿当前所有处于「等你处理」状态的任务，按通知规则各生成一条通知（事件序号用当时的 `latest_seq`，来源标 `backfill`），再把游标设为 `latest_seq`。只补当前等待中的，不补历史。
2. 「最近已通知状态」落库（`web.sqlite3` 新表或在通知行上记 `(kind, task_id, state_key)`），`commit` 时按它去重；**任务离开等待状态再进入时必须重新通知**（state_key 要包含进入的序号或上一次离开的序号，设计轮定）。
3. 协调既有测试：`notifier.test.ts:35`「首次启动已有事件时通知数为 0」与 `notifier-real.test.ts:28`「新面板库不补历史」改写为新契约（补当前等待中的、不补历史事件），不是删除；新增「重启后同状态新序号不重复」「离开再进入重新通知」两条。
4. 迁移：已有 `web.sqlite3` 升级时不重发历史。
## 不做什么
不改内核；不改铃铛界面；不改系统通知。
## 验收标准
真实账本用例：账本里有一个 `await_merge` 任务、空面板库启动 → 恰 1 条通知；重启面板并追加一条不改状态的 `task.updated` → 通知数不变；任务 `reject` 后再次到 `await_design_approval` → 新增 1 条。`npm test --prefix apps/web` 全绿，用例数只增不减。
## 参考
`apps/web/server/{notifier.ts,notifier-rules.ts,store.ts}`、`apps/web/test/notifier*.test.ts`、`docs/实现-web-sqlite3-与通知器/实现-web-sqlite3-与通知器.md`、T-38 定位文档 `docs/通知器首次启动不补发等你处理的任务/…-bug.md`（分支 `autome/通知器首次启动不补发等你处理的任务`）。
