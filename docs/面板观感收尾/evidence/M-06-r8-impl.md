# M-06 · 实现 #8 · 宿主快检修复

## 根因与修复

设计 C-Q2 将项目下拉改成标为「技能范围」的 SegmentedTabs 链接。宿主 F-1 已完成绑定、解绑、关闭抽屉，随后在 controls.spec.ts:257 等待已不存在的 combobox，90 秒超时。不是绑定权限失败，也不是等待时间不足。

同类检索 apps/web/e2e 的 combobox/selectOption 仅此一处。保留原 F-1 及其绑定隔离断言，改为在「技能范围」navigation 内点击「全局」链接；补项目→全局→项目双向切换、URL、aria-current、本项目筛选可见性断言。未改生产代码、超时或跳过设置。上一轮审计 M-05 通过，无新增退回检查；宿主失败检查本来就在项目测试体系中，本轮原地修复。

## 命令与结果

- 修前原样执行 `(cd apps/web && npx playwright test e2e/controls.spec.ts -g 'F-1 技能按仅当前项目绑定后只进该项目')`：初始化 `spawnSync ps EPERM`，未进入用例，不能称本地复现红；宿主原始红为 35 用例中 34 通过、F-1 超时。日志 `.autome/output/m06-r8-red.txt`。
- 修后同命令 exit 1，仍在初始化被同一限制阻断，未取得浏览器绿。日志 `.autome/output/m06-r8-after.txt`。不绕过进程清理检查，不扩大权限。
- M-06 验收 `(cd apps/web && npx vitest run test/skills-page.test.ts test/skills-model.test.ts test/ui-kit.test.ts test/ui-base.test.ts test/styles.test.ts test/testids.test.ts)`：**95/95 通过，较上一轮同命令 95 差 0**；6 文件数量不变。本轮只修改既有 F-1 步骤，未增删用例。日志 `.autome/output/m06-r8-accept.txt`。
- `(cd apps/web && npx playwright test --list e2e/controls.spec.ts)` 成功列举原 8 条 controls 用例，较上一轮差 0；列表不等于执行通过。日志 `.autome/output/m06-r8-list.txt`。
- 用户指定 `sh scripts/ci-quick.sh --since 3a1b9eea948cb70ee2d181b072c4610cb30ea6e6`：exit 1，12 步中 11 步通过，Web e2e 初始化/清理被 `spawnSync ps EPERM` 阻断。Format、Clippy、lint、typecheck、build 通过；related 没有匹配文件，不能算测试通过。日志 `.autome/output/m06-r8-quick.txt`。浏览器实际执行 0 条，较宿主 35 条少 35，原因是 setup 阻断，非删除或跳过用例。最终快检以宿主提交后复跑为准。
- 未运行全量门禁 `scripts/ci-local.sh`。`git diff --check` 通过。

## 自审清单

1. **情形表逐行**：C-Q1 固定高度/126 卡/描述与路径截断，C-Q2 分段链接/编码/当前项，C-Q3 单胶囊及空绑定，C-Q4 勾选块/禁用原因/描述来源顺序，由上述 95 条验收通过。此次 C-Q2 的真实导航步骤已更正，浏览器执行受阻，待宿主确认。
2. **失败分支**：验收覆盖失去授权、忙态禁用、纯项目技能禁全局、工具不可见、保存失败、空清单与空描述回退，均通过。读盘失败、超时、外部服务不可达不适用：本轮仅修改 e2e 定位，设计 C 节未要求这些 I/O 分支。
3. **全部迁移**：既有六角色×全局/项目×选中/取消回调、成功/无需修改/失败提示验收通过。F-1 原有绑定→保存→范围往返→解绑→保存→关闭断言保留；页面项目→全局→项目新增明确断言但未实跑，不冒充通过。未引入状态机。
4. **输入域边界**：验收中的空列表/空描述/空白/HTML 普通文本、空/单/六角色、126 项、含斜杠项目 ID 编码通过；新定位对项目 ID 使用 encodeURIComponent。日期与数值极值不适用：没有新增该类输入。

## 交接

M-06 保持待审，未推进新里程碑，implementation-round 为 8/65。宿主需复跑快检，尤其确认 F-1 双向范围导航和原绑定隔离断言。此轮没有浏览器通过证据。
