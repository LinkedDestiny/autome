# M-04 · 实现 #6（快检修复轮）

结论：修复两处 JSON 导入导致的 TS1543；指定快检本地通过，最终以 Autome 提交后重跑为准。本轮不推进里程碑，M-04 沿用上一轮的「开放」、reopen 0；implementation-round 为 6/30。

## 修复与回归

- 根因：settings-config.test.ts 与 settings-real.test.ts 静态导入 JSON fixture 未声明 JSON import attribute，不符合当前 NodeNext 类型检查规则。两处均添加 with { type: 'json' }，不改变 fixture、断言或产品逻辑。
- 同类检查：扫描 apps/web 的 ts/tsx/js/mjs 静态 JSON 导入，仅找到这两处；原始结果见 .autome/output/m04-r6-json-imports.txt。
- 上轮独立审计为 M-03 通过，没有审计缺陷检查需要搬入。本轮宿主发现问题的检查已在项目 package.json 的 typecheck 脚本内，直接原样复跑，未新增重复静态用例。

## 命令与结果

1. 修复前：npm run typecheck --prefix apps/web，退出 2，原样复现两处 TS1543。输出：.autome/output/m04-r6-typecheck-red.txt。
2. 修复后原样快检：

   cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web

   退出 0，所有检查通过；已有 node_modules，未安装依赖。输出：.autome/output/m04-r6-quickcheck.txt。
3. 直接相关测试：npm --prefix apps/web test -- test/settings-config.test.ts test/settings-real.test.ts，退出 0，2 文件、11 例全部通过，失败 0、跳过 0。输出：.autome/output/m04-r6-related.txt。

用例总数为 11；与上一轮相同两文件基线 11 相比差 0，没有新增、删除或跳过用例。与上一轮去重执行总数 50 相比差 -39，来自本轮仅运行被改动的两文件：未运行 HTTP 6、labels 8、styles 12、settings-onboarding 7、settings-onboarding-real 4、w9-layout 2。快检是静态检查，不计测试用例数。

## 自审清单

1. **情形表逐行**：本轮仅修复测试加载语法；两处 JSON 导入均经类型检查与 Vitest 运行通过。既有配置展开、警告、路径、错误隔离、名字确认、无删除调用守卫及真实内核移除等 11 例通过。本轮不重报 M-04 全部情形验收。
2. **失败分支**：TS1543 修复前红、修复后绿；配置读取错误、非法配置、名字不匹配、未结束任务拒绝等既有分支随这 11 例通过。HTTP Host/Origin 不在本轮子集，上一轮监听阻塞尚未重新取证。
3. **全部迁移**：不适用新增迁移，本轮未改状态机或产品逻辑；既有移除拒绝、取消后移除、事件刷新随 settings-real 通过。
4. **输入域边界**：不适用新增输入域，本轮仅补 JSON 模块属性；既有配置 null/缺省、名字空串及路径边界随 settings-config 通过。

## 范围与后续

未运行 sh scripts/ci-local.sh 或全量测试，未推进新里程碑，未将任何里程碑标已完成。M-04 完整 37 例验收仍待后续取证，尤其上一轮 HTTP 监听 EPERM 的 6 例；本轮快检通过不替代该验收。
