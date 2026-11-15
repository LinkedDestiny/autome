# M-08 · 实现 #15（快检修复）

结论：修复宿主快检的标签集中管理检查，M-08 保持待审，不推进新里程碑。implementation-round：15/65。真实面板等上一轮未完成的独立复验仍交审计；本轮不据静态检查关闭里程碑。

## 修复与回归

宿主在 6e7774a6ed 发现的检查为 `test/labels.test.ts > 7 views do not duplicate status, column or stage Chinese literals`。原检查已在项目测试体系，本轮逐字保留，无需复制第二份。上一轮审计是 M-07 通过，无新增审计退回检查。

原样复现命令：`(cd apps/web && npx vitest run test/labels.test.ts)`。日志 `.autome/output/m08-r15-red.txt`：10 例，9 通过、1 失败，断言原文 `env.tsx: 已停止`。根因是驱动卡片直接写入「已停止推进」，包含集中状态标签「已停止」。

修复：把驱动卡片的六项中文文案统一放入既有 `app/lib/admin-labels.ts` 的 ENV 中；env.tsx 引用常量。保持显示文字、状态判断、接口和交互不变。检查范围覆盖本次新增驱动卡片的标题、本面板标记、两种推进状态和两个时间标题；原标签守卫还遍历全部 routes/components 的 TSX，未放宽断言。

修复后：`(cd apps/web && npx vitest run test/labels.test.ts test/env-page.test.ts)`，日志 `.autome/output/m08-r15-green.txt`：18/18 通过（labels 10、env-page 8）。同口径基线也是 18，差 **0**，没有新增或删除测试；其中 labels 从 9 通过/1 失败变为 10 通过，env-page 维持上一轮 8 例。与上一轮里程碑验收 115 例相比，本轮定向集合 18 例差 -97，来自修复轮缩小执行范围，不是删减用例。

相关单测：`(cd apps/web && npx vitest related --run --project unit app/lib/admin-labels.ts app/routes/env.tsx)`，日志 `.autome/output/m08-r15-related.txt`：18 文件、150/150 通过。与上一轮 related 集合 489 例差 -339，原因是本轮只改两个文件且限定 unit，上轮包含 server/core 集成；无同口径上轮实跑基线，不将范围差异当作测试增减。不同命令覆盖有重叠，不累加为独立用例总数。

## 同一条快检

按本轮最后指令运行（未运行全量门禁）：

```sh
if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi
```

日志 `.autome/output/m08-r15-quick.txt`：退出 0，24 秒，Format、Clippy、Web install、Web lint、Web typecheck 五步通过。脚本明确报告「未提供本轮起点，未跑改动相关的测试」，所以不宣称已复跑宿主的 1060 例。相关 unit 与原失败检查由上述独立命令补跑。宿主提供起点后重跑的结果为准；没有改脚本、预算或测试配置。`git diff --check` 通过。

## 标待审之前的自审清单（本轮保持待审）

1. **情形表逐行**：E-3 本轮涉及的环境卡，本面板标记、推进中、停止推进均由 env-page 第 8 例逐项渲染检查通过；旧内核回退及读取失败原文同例通过。时间字段仍使用同一个 driver 的 started_at/last_tick_at，仅标题引用迁移；驱动登记和 CLI 行为未修改，不在本轮重新报验。
2. **失败分支**：env-page 第 1 例拒绝授权、读盘失败、请求超时、内核未连接，第 2 例 recipe 超时，第 4 例终端失败，第 8 例旧内核和读取失败均执行通过。空 intent 拒绝由第 3 例执行。外部服务不可达不适用：本轮没有外部服务调用。
3. **全部迁移**：本轮不新增状态机；现有 env.probing/env.changed 重验及无关消息不触发由第 7 例通过；驱动卡 driving 真/假两个渲染分支由第 8 例通过。内核登记→过期→恢复等迁移未改，仍以 r14 证据及后续独立审计为准，不声称本轮重新执行。
4. **域边界**：本轮只移动静态文案，无新增输入域；日期、数值极限不适用。现有空 intent、无 gateway、未检测数据、旧内核空列表、多个驱动均由 env-page 测试通过；未知标签空串、__proto__、toString 等由 labels 测试通过。

本轮未手工启动服务、浏览器或常驻内核，未接触真实账本；测试为 unit。未运行 ci-local，不启动下一会话。
