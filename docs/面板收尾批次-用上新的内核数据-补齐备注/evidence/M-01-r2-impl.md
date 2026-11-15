# M-01 实现 #2：宿主快检退回修复

## 范围与根因

本轮是修复轮，不推进新里程碑；M-01 保持待审，implementation-round 为 2/65。
宿主 f4d2f7c26a 快检的唯一失败为 `test/labels.test.ts > 7 views do not duplicate status, column or stage Chinese literals`：TaskChangesPanel 直接写入“已合并到默认分支”，包含受集中管理约束的阶段字面量“合并”。将完整提示移入已有 `CHANGES_COPY.merged`，组件引用该字段，显示内容不变。

没有上一轮审计。宿主抓到缺陷的检查已在项目测试体系中，逐字保留，不重复新增、不弱化断言；原有 task-changes 最后一例继续逐字断言渲染出的中文提示。labels 守卫遍历 routes/components 的全部 TSX，同类扫描范围没有缩小。

## 红绿与快检

- 红：`(cd apps/web && npx vitest run test/labels.test.ts test/task-changes.test.ts)`，19 个用例，18 通过、1 失败、0 跳过；失败正是宿主指出的第 7 条守卫。原始输出 `.autome/output/m01-r2-red.txt`。
- 绿：修改后运行完全相同命令，19 个用例全部通过。原始输出 `.autome/output/m01-r2-green.txt`。
- 用例总数较本轮修复前基线 19 差 0；labels 10、task-changes 9，既有检查原样复用。task-changes 与上一轮基线 9 差 0。上一轮宿主 related 总数 1000 属于更广范围，不能把本轮 19 个子集解释为删掉 981 个用例。
- 按本轮最后指令执行同一快检命令：`if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi`。退出 0，17 秒，Format、Clippy、Web install、Web lint、Web typecheck 五步通过；但脚本明确报告“未提供本轮起点，未跑改动相关的测试”，不能宣称重现宿主 1000 个测试全绿。原始输出 `.autome/output/m01-r2-quick.txt`。最终快检结论以 Autome 提交后重跑为准。
- 未运行全量门禁 `scripts/ci-local.sh`。本轮不重新推进 M-01，不重跑其整套验收；上一轮完整验收中的环境受阻项仍交独立审计。

## 标待审之前的自审清单（本轮保持待审）

1. **情形表逐行**：本轮影响 A-2 已合并提示；原有渲染用例验证 done/merged 显示原文、保留完成进度、隐藏待合并提示。新增/删除/修改、多仓库分组、改名来源、删除数口径等行由 task-changes 其余用例复验通过。A-1 文档与 A-3 事件未改，本轮不重复声明完成这些行的验收。
2. **失败分支**：task-changes 原用例分别检查读取失败 EACCES、授权拒绝、内核离线、请求超时原文显示；available=false 检查通过。本轮不新增授权、读盘、网络或超时分支。
3. **全部迁移**：task-changes 原用例遍历 await_merge/rebase/merge_gate/merging/main_check/cleanup 的 active 与 failed，另查 done；授权 true/false、默认标签与显式文档选择全部通过。文案字段本身无状态迁移，不适用额外状态机检查。
4. **域边界**：原用例覆盖空集合、单文件、删除阈值 (0,0)/(1,1)/(2,4)/(2,5)/(4,10)/(5,11)、二进制零行数、仅删行的修改文件，全部通过。前导零、闰日、跨年、最大数值不适用：本轮只提取固定字符串，没有此类输入域或解析改动。

## 相关测试与环境交接

`(cd apps/web && npx vitest related --run app/components/task-changes-model.ts app/components/task-changes.tsx)` 退出 1，选中 12 文件、66 个用例：59 通过、4 失败、3 未执行（Vitest 记 skipped）。较上一轮 related 的 339 个少 273，来源是本轮仅传入两个文案改动文件、影响选择范围缩小；没有增删测试或条件跳过。该 66 个集合没有相同参数的上一轮实跑基线，不把范围差声称为测试数量退化。原始输出 `.autome/output/m01-r2-related.txt`。

4 个失败均为 task-detail-real、task-docs-real、task-merge-real、task-timeline-real 的进程清理核对 `spawnSync ps EPERM`，各自 afterAll 同因报错；task-live-real 的 beforeAll 被 `listen EPERM` 阻断，其 3 个用例未执行。没有发现业务断言失败。未更改环境受阻用例，交宿主与审计复验。测试使用原有临时账本夹具并执行既有 teardown；本轮没有手动启动常驻服务，因 ps 被拒绝，不宣称已核实无残留。

`git diff --check` 通过。M-01 保持待审，未标已完成，未启动下一会话。
