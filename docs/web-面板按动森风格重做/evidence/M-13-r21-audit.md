# M-13 r21 集成复验（审计 #17）· 通过

范围：6c356f8（stage-panel 补齐三个原生导入的文件；设置路由测试改指 /loop?project=…&tab=project）。

1. 触发原因对应：发布包缺 app/lib/waiting.ts 等；`server/` 对 `app/` 的值导入只有 waiting、overview-model(toOverviewDetail)、task-columns(columnOf) 与 load-context，其余为 type-only，清单已覆盖。rebase 丢失提交 0 个。
2. 测试 diff：install-alignment 新增原生 Node 加载 server/overview.ts 的冒烟（加强）；settings-onboarding 仅换导入名为路由 action；w9-http 对新旧两个路径都断言 403，超长保存改打 /loop。`projects/:id/settings` 现为 302 重定向、无 action，故 405 属设计内；未削弱断言。
3. 冲突标记：`git grep` 对 6c356f8 结果为空。
4. 验收命令（overview / overview-summary / overview-states-real / labels）+ settings-onboarding：5 文件 62 用例全绿；w9-http 9 用例全绿。
5. 负向对照：用去掉三个新增文件的 stage-panel 暂存，原生加载 server/overview.ts 报 ERR_MODULE_NOT_FOUND（overview-model.ts）；正确清单暂存加载无错。临时脚本已删除，工作区干净。
未跑全量门禁与 install-alignment 整条（宿主合并前统一运行）。
