# M-05 · 实现轮 8/25 · rebase 冲突解决

本轮没有开放里程碑；五项均已由审计关闭。按用户追加要求完成 rebase，不创建新里程碑、不重新关闭里程碑，保留原状态。审计 #6 已通过，没有应逐字搬入且红绿复跑的缺陷检查。

## 修复与范围

- 执行 git rebase main，目标 main 为 7f29e3dd；在重放 4f18bf3b 时出现用户指定的四处冲突。逐一解决并 git add、GIT_EDITOR=true git rebase --continue，44 个重放步骤全部完成。未 merge、未切分支、未推送。
- bell-targets.ts：任务和项目目标都开放。
- routes.ts：同时保留项目看板、项目下新建任务以及 W8 任务日志页路由。
- app.css：保留两边独立的看板、创建页与日志页样式。
- bell-view.test.ts：默认任务和项目都有链接；显式关闭两类目标时仍断言无链接与降级提示；无归属和 ID 编码断言保留。未减少用例。
- 设计说明就地同步项目目标开放的事实；实现轮次写 8/25。冲突解决已随 rebase 重写进原功能提交，本轮收尾提交保存证据与当前设计。
- git merge-base --is-ancestor main HEAD 退出码 0；无未解决冲突。

## 命令与结果

原始日志仅位于 .autome/output/。

1. npm test --prefix apps/web -- test/session-log-view.test.ts test/task-page.test.ts test/bell-view.test.ts test/styles.test.ts：4 文件、31 用例全部通过，失败/跳过 0，退出码 0，r8-m05.txt。
2. npm test --prefix apps/web -- test/session-log-view.test.ts test/task-page.test.ts test/bell-view.test.ts test/styles.test.ts test/bell-system.test.ts test/labels.test.ts test/board.test.ts test/task-new.test.ts：8 文件、64 用例全部通过，失败/跳过 0，退出码 0，r8-tests.txt。
3. npm run lint --prefix apps/web：退出码 0，r8-lint.txt。
4. npm run typecheck --prefix apps/web：退出码 0，r8-typecheck.txt。
5. npm run build --prefix apps/web：退出码 0，r8-build.txt。
6. git diff --check：通过。rebase 记录：r8-rebase.txt、r8-rebase-continue.txt。

用例总数为 64 个不同用例，重复跑 M-05 不重复计数。相比上一实现轮 50，净增 14：不重跑未改动 Rust 队列 6 个，增加看板/创建页 18 个，以及 main 已增加的 styles 2 个，50 - 6 + 18 + 2 = 64。M-05 同命令基线从 29 增为 31，差 +2 来自看板和新建页内部滚动样式测试。本轮未新增测试数量，保留并合并已有断言。

未运行全量门禁或宿主快检组合；上述 lint/typecheck/build 是 M-05 明定的独立验收。真实浏览器 H-01–H-05 未取证，仍待合并前用户确认。

## 标待审之前的自审清单

本轮保留审计已关闭状态，仍按冲突范围执行四项自审。

1. **情形表逐行**：M-05 六项视图检查（收起 SSR、展开/空态、暂停不滚动不聚焦、跳到最新与容差、源码焦点守卫、中文文案）通过；任务页五项（路由、单次只读调用、顺序和无会话、错误提示、事件重验）通过；默认任务/项目链接、显式关闭目标降级、无归属和编码通过；日志与 main 看板/创建页样式共存，样式守卫通过。
2. **失败分支**：任务页 gateway null、503/504、not_found/404、500 分支均通过；通知权限拒绝/不支持、无列表/空正文降级通过；看板 503/409 与创建页校验拒绝由相关子集覆盖。服务端日志读盘失败、授权拒绝、超时不适用本轮路由/样式合并，未改服务端，不宣称重验。
3. **全部迁移**：日志暂停→追加不移动、跳到最新→恢复滚动通过；通知开启/关闭与目标可用/不可用、看板全部过滤器迁移由子集检查通过。rebase 未改变日志状态机或 WebSocket 通道；其它后端状态迁移不适用。
4. **域边界**：无会话、无归属通知、空正文、ID 编码、滚动 4px 容差两侧、窗口前后边界通过；看板日期边界、创建页输入限制保留其既有测试并通过。日志 offset 最大值/UTF-8 切块不适用本轮冲突修改，未重跑。

宿主下一步复跑快检和合并前全量门禁。
