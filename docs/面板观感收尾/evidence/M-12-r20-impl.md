# M-12 · 实现 #20

范围：B-Q4 / BL-01，只推进 M-12。上一轮审计 M-11 已通过，没有退回缺陷或需迁入的审计检查。

## 实现

LoopView 使用现有 changedFields 生成的当前角色 patches 判断 dirty，独立于保存按钮禁用与历史成功提示。RoleDrawer 的添加技能链接复用 window.confirm；拒绝只 preventDefault，不更改草稿或关闭抽屉；同意沿用原 Link 与编码后的 project 参数。现有保存成功 effect 清除已保存角色草稿，失败保留草稿。

## 命令与结果

- 基线：`(cd apps/web && npx vitest run test/loop-page.test.ts test/loop-defaults.test.ts test/ui-kit.test.ts)`，3 文件、49 用例通过。输出 `.autome/output/m12-baseline.txt`。
- 最终同命令：3 文件、56 用例通过，0 失败、0 跳过；相比本轮修改前基线 +7（B-Q4 六情形各一例，五字段与全局路径边界一例）。loop-page 26、loop-defaults 9、ui-kit 21。输出 `.autome/output/m12-tests.txt`。
- 负向对照：将确认条件临时改为 false，55 用例中的脏草稿取消、确认、保存失败 3 例失败，其余 52 通过；输出 `.autome/output/m12-negative.txt`。恢复生产代码后增加域边界用例，再运行上述最终命令，56 例全绿。没有保留缺陷注入。
- 用例通过渲染生产 LoopView、拦截生产 Link 的 onClick，调用真实处理器并断言 preventDefault；保存成功/失败执行生产 effect。未运行真实浏览器导航或系统弹窗，未跑全量门禁。

## 标待审前自审清单

1. **情形表逐行**：无改动→无提示；脏草稿取消→提示且 preventDefault、草稿不变、抽屉仍渲染；脏草稿确认→提示且不阻止 Link、项目参数编码不变；改回原值→无提示；保存成功→effect 清草稿后无提示；保存失败→effect 保留草稿并继续提示。六条均由新增用例执行通过。
2. **失败分支**：本条款点名保存失败已覆盖（saved 为空、error 存在），确认取消已覆盖。授权/读盘/超时/外部不可达不新增分支，本改动不发请求；既有 loop-defaults 失败路径用例在验收命令中通过。
3. **全部迁移**：干净→直接导航；改动→脏；脏→取消后仍脏；脏→确认允许导航；脏→改回原值变干净；脏→保存成功清草稿；脏→保存失败保持脏。以上各路径通过；确认只放行链接，不模拟目标页加载。
4. **域边界**：五个角色字段（enabled/runtime/model/effort/skills）逐一变化均提示；空模型、null effort、空/单技能数组均检查；全局路径 `/skills` 与含中文、空格、`/?&` 的 project 参数检查通过。无数值/日期输入域，前导零、闰日、跨年、最大值不适用。

## 指定快检

已执行 `sh scripts/ci-quick.sh --since 5b340aaee5168a2c6ce2e8d6a39f8dad9403ce9b`：21 步中 20 步通过，Web e2e 因 `spawnSync ps EPERM` 初始化及清理失败；静态检查、相关单测、内核/Web 构建通过。输出 `.autome/output/m12-quick.txt`。浏览器用例未实跑通过，需宿主复验，不视为协议失败。快检单测总数 101，动态选择范围不同于上一轮 M-11，不据此声称与上一轮同范围增加；本里程碑同命令基线与增量为 49→56（+7）。

最终补充 `npm run lint && npm run typecheck` 通过（`.autome/output/m12-final-static.txt`），`git diff --check` 通过。未增加视觉布局；浏览器视觉检查未执行，沿用本任务宿主/人工验收边界。
