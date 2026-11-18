# M-06 · 实现 #7

## 实现与范围

按 C-Q1…C-Q4 收紧三列技能卡：196px 固定高度、三行描述、首条路径和额外来源数；范围用链接分段；绑定角色汇成一枚胶囊。抽屉保留完整描述与全部来源，角色使用两列 Check。Check 的禁用原因移进 label，保留 aria-describedby 与完整 title。未改写绑定权限或保存协议。

上一轮审计 M-05 通过，没有退回缺陷检查需要搬入。本轮新增回归留在现有测试文件。

## 命令与结果

- 基线：`(cd apps/web && npx vitest run test/skills-page.test.ts test/skills-model.test.ts test/ui-kit.test.ts test/ui-base.test.ts test/styles.test.ts test/testids.test.ts)`，89/89 通过（`.autome/output/m06-baseline.txt`）。这是上一轮提交状态下的同命令实测基线。
- 最终同命令：95/95 通过，较基线 **+6**：skills-page 从 10 到 15（四条设计契约 + 一条六角色×两个范围×勾选/取消回调）；skills-model 从 16 到 17。其余 21/12/15/15 不变。设计预期的 ui-base 11 已由前轮增至 12，故不以设计最低总数代替实测。
- 直接相关子集：上述六文件加 `test/add-project.test.ts test/queue-deps.test.ts`，120/120 通过（`.autome/output/m06-final.txt`）。相对同范围上一轮基线 114 增加 6；其中额外两个既有文件共 25 个用例未增减。
- `npx playwright test --list e2e/layout.spec.ts` 成功列举 16 条布局用例（`.autome/output/m06-related.txt`）。技能 fixture 改成 126 条长描述，补等高、至少 9 张完整可见、三行截断断言。**仅列举，不是浏览器执行证据**；H-03/H-04 与浏览器布局仍由宿主/用户验收。
- 三项负向对照均 exit 1：删三行截断→固定卡用例失败；恢复逐角色胶囊→单胶囊用例失败；原因移出 label→ui-kit 可追溯及抽屉结构用例失败。输出分别在 `.autome/output/m06-negative-{clamp,binding,reason}.txt`。全部还原，最终验收再次通过。
- 初次开发测试 91/94 通过：旧路径全量展示断言、分段组件空格预期、Check 回调树路径不匹配；依照新设计更新断言后全绿，未删用例。

## 标待审之前的自审清单

1. **情形表逐行**：C-Q1 的 126 卡/196px/三行/单行路径/另 1 处由 skills-page 新增用例验证；完整描述在抽屉内全文保留。C-Q2 全局与编码项目链接分别有当前标记，二级筛选带提示。C-Q3 空绑定与两角色分别验证描边/单枚实心胶囊，模型覆盖六角色文本。C-Q4 六个 Check、两列、原因在同一 label、可追溯 ID、描述→来源→范围→角色→footer 均逐项通过。浏览器几何只补断言，未声称实测。
2. **失败分支**：失去授权/忙态禁用保存与打开目录、项目技能禁全局、工具不可见禁角色、保存失败 alert、无修改和保存成功 status 均由既有 skills-page 用例通过。空清单与空描述回退通过。读盘失败、超时、外部服务不可达不适用：本里程碑未改 I/O，设计 C 节未点名这些分支。
3. **全部迁移**：六角色×全局/项目×选中/取消共 24 次实际回调执行通过；全局/项目可见性与绑定变化由 skills-model 既有用例通过；Segmented 两方向回调与 Check 原生两方向由 ui-kit 通过；成功/无改动/失败结果由 footer 用例通过。本轮不新增状态机。
4. **输入域边界**：空列表、空搜索/无匹配、描述 undefined/null/空串/空白/HTML 文本、单角色/空角色/六角色、两来源、126 项、项目 ID 含斜杠编码、超长多行描述均通过。日期、数值最大值不适用：本轮无此输入域。

## 限制

未运行全量门禁或真实浏览器，未拍截图。原始输出只写 `.autome/output/`，未改配置或规则。最终快检结果见下。

## 快检与提交

- 已执行用户指定 `sh scripts/ci-quick.sh --since 154805a0b0a84af0933232ca99f74513f023c28e`。最终 exit 1，38 步中失败 3 步：shell-live、task-live-real、Web e2e；均受 `listen EPERM 127.0.0.1` 或 `spawnSync ps EPERM` 限制，包括 e2e 清理。其它 35 步通过，包括 lint、typecheck 和相关单元测试。原始记录 `.autome/output/m06-quick-final.txt`。宿主需复跑这三项，不将环境失败记为通过。
- 首次快检有 5 步失败：上述环境项，加测试正则多余转义 lint（已修），以及与删 clamp 负向对照重叠的 skills-page（检查隔离失误）。最终串行重跑已排除后两项。没有运行 `scripts/ci-local.sh`。
- `git diff --check` 通过。M-06 标待审，轮次 `7/65`，其它里程碑未改。
