# M-03 实现轮 4

## 范围与修复

按 A-Q4/A-Q5 统一丝带 17/13px 与标题继承，删除总览及看板标题字号覆盖；新增 Drawer，角色和技能抽屉共用标题关闭按钮、独立滚动正文、固定 footer。角色保存/恢复及结果、技能保存及结果在 footer，技能来源前移到勾选之前。新增 dirtyText 纯函数并接入 oneScreen；布局清单增加两个抽屉，注释同步到 35 条总清单。焦点管理留在 M-13，不扩大本轮范围。

上一轮 M-02 审计通过，没有实现缺陷退回，因此无须迁入审计缺陷回归检查。

## 命令与结果

- 改前同范围既有九个测试文件：129 个用例通过；日志 `.autome/output/r4-baseline.txt`。新增 dirty-text 文件尚不存在，故基线命令不包含它。
- 最终验收命令：`(cd apps/web && npx vitest run test/ui-base.test.ts test/dirty-text.test.ts test/ui-kit.test.ts test/styles.test.ts test/loop-page.test.ts test/skills-page.test.ts test/testids.test.ts test/board.test.ts test/overview.test.ts test/test-groups.test.ts && npx playwright test --list e2e/layout.spec.ts | grep -cE '^  layout\.spec\.ts:.*› 1512×944 ')`。
- 结果：10 文件、137 用例全部通过，0 失败、0 跳过，布局计数 16；日志 `.autome/output/r4-accept-final.txt`。同范围增加 8：ui-base +4、dirty-text +2、loop-page +1、skills-page +1；布局从 14 到 16（只列举，未实际执行）。上一轮 M-02 为 87，本轮比上一轮 +50，其中范围变化 +42，实际新增 +8，并非全部都是新增覆盖。
- 旧名检索：`grep -rnE 'skills-drawer-body|overview-heading h1|board-column \.ribbon h2' apps/web/test apps/web/e2e apps/web/scripts`，改前匹配 1（testids 白名单），改后 0；日志 `.autome/output/r4-legacy.txt`。样式原有两条标题覆盖也已删除。
- `git diff --check` 通过。

## 标待审前自审清单

1. **情形表逐行**：A-Q4 页面 17px、区块 13px、h1/h2/h3 继承、fit-content 四点由 ui-base 丝带测试逐项检查；A-Q5 dialog 语义、标题行关闭、正文滚动、固定 footer、角色项目恢复/全局无恢复、技能来源顺序、两种结果提示均由新增 SSR/CSS 测试通过。两条布局用例已列举，实际控件外观和 footer 边界断言已写入 e2e，未冒充浏览器通过证据。
2. **失败分支**：角色保存失败、技能保存失败均断言 alert 在 footer；原有失去授权/忙态技能绑定与打开目录禁用、角色写入未授权禁用、离线加载失败和无网关保存失败测试继续通过。读盘失败、超时不是本里程碑新增或点名的分支，不适用。四种脏文本分别检测，正常文本负例通过。
3. **全部迁移**：Drawer 关闭按钮和 Escape 各调用 onClose 一次，Enter 不关闭；角色全局/项目 × 无结果/保存成功/保存失败，技能无结果/保存失败/无需改动/保存成功/目录打开消息均经 SSR 测试通过。页面选择卡片打开与父组件关闭回调路径保留；新增两个 e2e 覆盖打开步骤，真实浏览器未跑。绑定范围业务状态机未修改，其既有全局/项目与禁用用例通过。焦点移入/恢复明确属于 M-13。
4. **输入域边界**：dirtyText 对空串、中文紧邻、重复字面量、nullable、NaNa、undefinedValue、null_value、合法数字逐项通过。标题/技能描述继续由 React 文本渲染；原有空描述与脚本字符串转义用例通过。本轮不涉及日期、数值范围，前导零/闰日/最大值不适用。

## 负向对照

逐项临时注入后立即还原；最终验收在全部还原后再次执行为绿：

| 注入与检查 | 红结果 |
|---|---|
| 追加 `.overview-heading h1 { font-size: 22px }`，跑 ui-base | 其它字号测试失败，exit 1 |
| 把关闭按钮挪到 header 之外，跑 ui-base | Drawer 结构测试失败，exit 1 |
| 从 dirtyText 正则删除 null，跑 dirty-text | 四种字面量测试失败，exit 1 |
| 删去角色抽屉 cases 项，Playwright --list | 布局计数 15，不能满足 16 |

日志：`.autome/output/r4-negative-{ribbon,close,null,layout}.txt`。过程中测试分组先登记文件、文件尚未创建时曾出现 2 个文件缺失失败，创建文件后最终 137 个全部通过。

## 限制

本轮未跑全量门禁、未取得浏览器或截图证据；指定快检自动触发 e2e，但初始化被沙箱阻断。H-03/H-08 的真实布局及视觉验收仍由宿主/人工覆盖。仅列举 e2e 不等于实际通过。

## 指定快检

`sh scripts/ci-quick.sh --since 664d234313be3aed29a601e42a19b7b23789c2a4` 已执行（日志 `.autome/output/r4-quick.txt`）。lint、typecheck 通过；83 批测试共 797 个通过（与验收存在重叠，不与 137 相加；上轮快检同范围基线未采集，不能给出可比增量）。执行 95 步、失败 1 步：Web e2e globalSetup/teardown 的 `spawnSync ps EPERM`，35 个浏览器用例全部未执行；另外相关测试超出快检 120s 预算留待合并门禁。此为环境阻塞，不宣称快检整体通过。负向对照期间快检也在运行，快检没有出现测试断言失败；全部还原后的独立验收 137 用例与布局清单 16 已重新通过。
