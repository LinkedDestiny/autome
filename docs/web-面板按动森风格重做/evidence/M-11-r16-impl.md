# M-11 · 实现 #16 · 缺值修复，截图环境阻塞

## 修复与结论

修复总览事实条与会话胶囊的设计/实现轮次：每个缺失字段显示「—」，保留真实零值；同类里程碑进度文案也逐字段回退。截图夹具在创建演示任务前取消共享夹具的未终结任务，保留已完成历史，让一个等待批准任务和一个进行中任务同屏；截图断言由 DOM 可见改为整张卡片在视口内，并禁止 undefined 文案。

截图生成在启动服务之前被 `spawnSync ps EPERM` 阻断，未替换 docs/images 的旧图；布局验收也在 setup/cleanup 被同一权限阻断。M-11 保持开放，reopen 保持 2，不标待审、不关闭。未改其它里程碑。

## 审计回归与命令

- 审计检查从 `.autome/output/overview-undefined.check.test.ts` 搬进 `apps/web/test/overview.test.ts`，复用文件已有 imports，测试体先逐字运行：`(cd apps/web && npx vitest run test/overview.test.ts)`，18 例中 1 败、17 过，日志 `.autome/output/m16-red.txt`。
- 原检查自身漏了必填 task.id：修好轮数字段后，原断言仍被 `/tasks/undefined` 干扰（`.autome/output/m16-green.txt`）。仅为该夹具补 `id: 'T-audit'`，原始 JSON 序列化断言不变；没有为错误夹具修改产品链接语义。补齐后暂时恢复旧 overview-labels.ts 再跑同一命令，19 例中 2 败、17 过，日志 `.autome/output/m16-red-valid-fixture.txt`；随即还原修复。第二个失败是新增的轮次边界回归。
- 绿：`(cd apps/web && npx vitest run test/overview.test.ts test/readme-images.test.ts test/docs-web.test.ts test/styles.test.ts test/labels.test.ts test/testids.test.ts)`，6 文件 66 例全过，日志 `.autome/output/m16-green-final.txt`。
- 相关测试：`(cd apps/web && npx vitest related --run app/lib/overview-labels.ts)`，15 文件 133 例全过，日志 `.autome/output/m16-related.txt`。
- `npm run build --prefix apps/web` 通过，日志 `.autome/output/m16-build.txt`。
- `npm run screenshots --prefix apps/web` 失败：ps EPERM，服务未启动，未产生新图；日志 `.autome/output/m16-screenshots.txt`。
- M-11 原验收命令 `(cd apps/web && npx vitest run test/readme-images.test.ts test/docs-web.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts)`：vitest 6 例全过，layout 14 条未执行，setup/cleanup ps EPERM；日志 `.autome/output/m16-layout.txt` 保存浏览器部分。
- 用例总数：M-11 契约仍 20（6 + 14），与上一轮审计差 0；实际执行 6，比上一审计 20 少 14，原因是浏览器前置失败。overview 从 17 增至 19，增加审计缺值回归及一条覆盖四角色 × 缺失/null/0/1 的边界用例。绿命令 66 的同选集旧基线为 64，差 +2；related 同选集旧基线 131，现 133，差 +2。不同命令存在重叠，不将其相加当作唯一用例总数。
- 未运行全量门禁。

## 标待审之前的自审清单（本轮未达到待审条件）

1. **情形表逐行**：五图引用及目录精确集合、五图 PNG/1512×944、Loop/技能非空、截图脚本文档均由 6 条契约测试通过。等待卡片轮次缺失回退与设计/实现会话预算分别由 overview 检查通过。总览黄/青两卡整张入视口已写成截图断言，但受 ps 权限阻断未执行；14 条全站一屏巡检同样未执行。旧总览图片仍有审计指出的缺陷，不宣称视觉通过。
2. **失败分支**：详情读盘失败、内核离线由 overview 原有用例通过；缺轮数字段先红后绿。截图进程枚举失败实际触发并记录。授权拒绝、外部服务超时不适用，本轮未改写操作或网络契约。缺图与去掉 data-scroll-region 的负向对照未重新注入，布局环境不可用，不记通过。
3. **全部迁移**：不适用，本轮未改产品状态机或多通道传输；等待/运行各状态渲染沿用 overview 全节点/状态枚举检查通过。截图夹具的取消操作仅调用既有 CLI，实际执行仍待环境放行。
4. **域边界**：设计与实现预算分别测试 undefined、null、0、1，零值不误判缺失；完整预算保持 0 / 15。原有零/单里程碑、无计费与已计费零检查通过。截图尺寸与文件集合边界通过；本轮未增加日期或字符串输入域。

## 交接

在具备进程枚举权限的环境运行 `npm run screenshots --prefix apps/web -- --out .autome/output/screens-M11`，核对总览黄/青卡均完整可见且无 undefined；将五图复制到 docs/images，重跑 M-11 完整验收。不得使用旧图或旧审计布局结果代替本轮证据。

## 宿主快检与提交

`sh scripts/ci-quick.sh --since 44464ad04793ea4396dee19ae8756b014a103ad3` 已运行，27 步中 26 步通过、1 步 Web e2e 失败，耗时 94 秒；Format、Clippy、Web lint/typecheck/build 与选中的相关测试通过，浏览器仍为 ps EPERM。日志 `.autome/output/m16-quick.txt`。未放宽快检或进程清理预检。
