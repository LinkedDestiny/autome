# M-02 · 实现 #3

结论：本里程碑验收 5 文件、43 用例通过；直接相关回归 3 文件、18 用例通过。四项负向对照均产生断言失败，已全部还原。本轮只推进 M-02，提交审计，不自行关闭里程碑。

## 实现与修复

- 新增页头、阶段条、失败与等待各自的纯模型与 TSX，接入任务路由，沿用现有 CSS token、项目链接及 W6 cardView 的 activity/gate。
- 需求里程碑与设计/实现轮次使用状态块；实现预算优先 task.budget_n；无状态块时取最后一条 snapshot/metrics。Bug 修复预算退到项目配置，复现状态只读业务事件。
- failed/stopped/paused/queued 保持对应节点；取消从取消事件之前最近一条带节点的状态恢复，包含 queued.resume；草稿取消不高亮。
- failureText 保留全文，覆盖内核 11 个原因种类及门禁超时独立文案；通知短文案未改。恢复建议按原因分支展示；按钮留 M-05。T-70 stale/waiting_for 缺省兼容。
- 自审发现真实 task.get 的项目名称为 project.name，而 W6 的 project.get 使用 display_name。先在 task-header 第 1 条加入已有真实 feature.json 的断言，原实现失败；修复为 name 优先、兼容 display_name 后全绿。红灯输出：`.autome/output/M-02-r3-project-red.txt`（3 条，1 失败、2 通过）。这是本轮自审发现，不是上一轮审计退回。
- 上一轮审计 M-01 通过，明确无需搬运的临时检查；本轮没有审计缺陷待回归化。

## 命令与用例基线

1. `cd apps/web && npx vitest run test/task-stage.test.ts test/task-notice.test.ts test/task-header.test.ts test/labels.test.ts test/task-card.test.ts`
   - 退出码 0，5 文件、43 用例，43 通过、0 失败、0 跳过。输出：`.autome/output/M-02-r3-accept.txt`。
   - 本里程碑同范围旧基线为 labels 8 + task-card 12 = 20；新增 stage 12 + notice 8 + header 3 = 23，总数增加 23，达到设计的 43。
2. `cd apps/web && npx vitest run test/task-detail-data.test.ts test/task-page.test.ts test/styles.test.ts`
   - 退出码 0，3 文件、18 用例，18 通过、0 失败、0 跳过；既有 18 条未增删。输出：`.autome/output/M-02-r3-related.txt`。
3. `node .autome/output/M-02-r3-mutations.cjs`
   - 驱动退出码 0。四次缺陷注入各自 Vitest 退出码 1 且为 AssertionError：实现预算优先 impl_round_limit；取消直接 stageOf 当前状态；取消只找 active；失败文案改成 renderReason。输出分别为 `.autome/output/M-02-r3-negative-{budget,cancel-direct,cancel-active-only,truncated-failure}.txt`。每次 finally 还原源文件，之后重新跑上述两条命令全绿。
4. `git diff --check`：通过。

本轮最终正向验收合计 **61 个不同用例**，同范围旧基线 38 → 61，增加 23。与上一审计轮 M-01 的 22 条相比净增 39：保留同跑的 data/page 6 条，新增本轮 23 条，另选 labels/card/styles 32 条，未重跑 M-01 events/docs/real 的 16 条（23 + 32 - 16 = 39）。上一实现轮 #2 只跑静态快检，Vitest 为 0，本轮相对其执行数增加 61。负向注入的重复执行与项目名红灯不计入不同用例总数。没有删除测试或新增跳过。

## 标待审之前的自审清单

### 1. 情形表逐行

- 需求进行中：stage #1 校验实现阶段、已走阶段数、三种里程碑状态、当前里程碑、预算优先级，过。
- 等批准：stage #2 当前批准设计，过。
- 审计失败：stage #3 当前审计并附失败状态，过。
- 已完成且状态块清理：stage #4 最后快照里程碑、metrics 轮次、全部完成，过。
- 已停止/已取消：stage #5 停在原节点，五种历史状态逐个恢复；批准设计后 queued.resume=implement 再取消停在实现；取消之后的节点不污染；草稿、首次排队、空历史不高亮，过。
- Bug 定位中/等补充信息：stage #6/#7，过。
- Bug 修复中：stage #8 四种复现业务结果分别检查，后续 fix_gate 非零不覆盖为新的“符合预期”，过。
- Bug 预算失败/完成：stage #9/#10，过。
- 修复预算配置回退/语义高亮：stage #11/#12，过。
- 失败全文/建议：notice #1/#2 十二行文案逐行对照 CLI render.rs 的 failure/next_step，并补 setup；长协议原文渲染不截断，过。
- 冷却/重试/门禁排队/排队等槽位/T-70 过期与等待/停止：notice #3—#8，过。
- 页头类型/id/标题/项目链接/分支/费用/状态：header #1—#3，真实回包项目名、零费用、无缺失费用标注、cardView 等价及整页 SSR 接线，过。

### 2. 失败分支

- design_budget、impl_budget、fix_budget、gate_same_failure 普通及超时、infeasible、protocol 长原文、session_crashed、rebase_conflict、config、core_step、setup：notice #1/#2 逐分支运行通过，未知原因原 kind 保留。
- 状态块读取失败与空状态块：stage #1 运行通过；复现不可执行/超时/未复现：stage #8 运行通过。
- 冷却匹配失败退重试、T-70 字段缺失：notice #4/#7 运行通过。
- 内核离线/503/504/not_found/404/其它读取错误：直接相关 task-page 既有用例运行通过。
- 拒绝授权不适用：本里程碑只有只读展示，没有写操作，M-05 验收；文档文件读盘失败不适用：M-04 文档展示范围，本轮未动文档读取。

### 3. 全部迁移

本轮不修改内核状态机，以展示映射验证所有节点 × 状态：stage #3 遍历 FEATURE_STAGES/BUG_STAGES 的每个节点，分别断言 active.node、paused.resume、queued.resume、stopped.at、failed.at 的当前阶段和前序完成数量；stage #4/#10 断言两种任务 done；stage #5 逐个断言五种有节点状态 → cancelled，并含 queued 无 resume/草稿 → cancelled。定位/补充信息/修复/复核/合并等节点均在遍历内。复现历史的新结果覆盖旧结果、后续门禁不改业务结果由 stage #8 验证。全部通过。

### 4. 输入域边界

- 空历史、空状态块、空/未知节点（含 __proto__）、单里程碑、缺失预算回退、预算 0 不被真值判断覆盖、修复轮 0：stage #1/#5/#11，通过。
- 零会话/零费用、全部会话有计费、含缺失计费：header #1/#3，通过。
- 超过 60 字的原因全文、未知/空原因种类、排队首位和较大位置、缺失/为 null 的 merge/stale、冷却截止时间精确匹配与不匹配：notice #1/#3—#7，通过。
- 前导零、闰日、跨年、输入最大长度不适用：本轮不解析用户数值或日期输入，不新增输入框；时间格式复用既有 labels，写入边界由 M-05 覆盖。

## 限制与交接

按本轮指令未运行全量门禁、宿主快检、全量 Vitest。lint/typecheck 由宿主随后运行。未做真实浏览器视觉验收，H-01/H-04 仍留人工清单；本轮有 SSR 内容与 aria-current 证据，不将其当作真实布局验证。未改内核、未切分支、未推送、未启动下一会话。
