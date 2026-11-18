# M-05 · 实现轮 6

## 范围与修改

仅推进 M-05（B-4、B-5、B-6），起点 c356f4e3d9deb8b8361fb301e3582919ed607ca8。上一轮审计 M-04 通过，没有缺陷检查需要搬入回归体系。

- 角色抽屉已绑定技能使用名称胶囊，保留移除操作；空集合显示「还没有绑定技能」；添加技能使用 btn btn-sm，保留 URL 编码后的项目参数，模型继续使用全站基础外观。
- 四项默认值统一标题、控件、恢复三行结构；可见保存文字统一、可访问名按字段区分；已保存位于对应卡的控件行。Stepper 新增可选隐藏可见标签，仍保留 label 与 input 的关联，其他调用保持默认行为。
- 项目信息三个区块改为自然高度，仅整体容器滚动；编辑器原有内部滚动保留；同步改写滚动白名单和守卫。既有 e2e「项目设置」追加三个区块不裁切及移除输入框滚入后完整可见断言，未改用例名。
- 草稿跳转确认属 M-12，焦点管理属 M-13，本轮未提前推进。

## 命令、总数与基线差

- 验收命令：`(cd apps/web && npx vitest run test/loop-page.test.ts test/loop-defaults.test.ts test/w9-layout.test.ts test/settings-config.test.ts test/testids.test.ts test/labels.test.ts)`。
- 本轮起点同命令实测 70；实现后 73，增加 3（loop-page 技能内容 1、loop-defaults 统一卡结构及反馈 1、settings-config 整体滚动结构 1）。分布 19 + 9 + 4 + 15 + 15 + 11，0 失败、0 跳过。上一轮 M-04 是不同集合 71，不能把集合变化当成新增或漏跑。
- 直接相关子集：上述六文件加 `test/ui-kit.test.ts test/styles.test.ts`，110 用例通过；比本轮验收多 37 个既有共享控件与样式用例，无新增。日志 `.autome/output/r6-final-tests.txt`。
- 基线与验收日志：`.autome/output/r6-baseline.txt`、`r6-accept.txt`。首次新增技能胶囊断言因 React SSR 文本分隔注释失败，剔除注释后检查可见文字，产品实现未为测试修改。
- 旧守卫检索：`grep -rnE 'settings-card-body|保存\{field.label\}' apps/web/test apps/web/e2e apps/web/scripts`，改前 2 行、改后 3 行，均为新的自然高度/无独立滚动断言。日志 `r6-old-before.txt`、`r6-old-after.txt`。
- 负向对照四次均 exit 1：保存恢复字段后缀；删除技能空态；卡片恢复 flex: 1 1 0 和内部 overflow；给三块恢复 data-scroll-region。分别触发 loop-defaults、loop-page、w9-layout、settings-config 失败，全部原样还原。日志 `r6-negative-{save-label,empty-skills,card-scroll,scroll-marker}.txt`，汇总 `r6-negative-summary.txt`。

## 标待审之前的自审清单

1. **情形表逐行**：B-Q3 全局四卡（无来源/恢复）、项目四卡（与全局相同/项目值、逐项恢复）、未修改禁用、保存成功就地反馈均通过 loop-defaults；B-Q4 全局 / 项目添加技能链接、空 / 单个 / 多个绑定、模型无专用外观类通过 loop-page；B-Q5 三块同容器、区块自然高度、textarea 仍内部滚动通过 settings-config、w9-layout、testids。设计的草稿六情形明确留给 M-12。
2. **失败分支**：loop-defaults 既有「离线、拒绝授权、读盘失败、超时、技能拒绝原样保留」与空输入不发请求用例全通过；settings-config 的配置失败独立显示（解析错、拒绝、读盘、超时、离线）全通过。布局 e2e 已补，真实浏览器未跑，不以静态结果代替视觉证据。
3. **全部迁移**：本轮未新增状态机；既有默认值合法写入→成功、项目值→null 恢复、非法/离线/拒绝→错误由 loop-defaults 请求断言覆盖；共享 Stepper 减少/增加、输入变更、边界钳制由 ui-kit 覆盖；抽屉全局/项目保存成功和失败提示由 loop-page 覆盖。新 UI 结构对这些路径一致应用。草稿取消/确认状态迁移属于 M-12，不适用本轮。
4. **域边界**：四字段 min/max、越界、小数、NaN、Infinity、空串/空格、前导零、u32 溢出均执行既有 loop-defaults 用例；技能集合空/单/多及包含中文、空格、斜线和问号的项目 ID 编码执行新增 loop-page 用例。日期边界不适用，本里程碑没有日期输入。

## 环境与交付

浏览器裁切检查由宿主执行；未运行全量 ci-local，也未重拍 README 或人工截图。只使用本里程碑验收、直接相关控件/样式子集与用户指定快检。

指定快检 `sh scripts/ci-quick.sh --since c356f4e3d9deb8b8361fb301e3582919ed607ca8` 已执行，负向变更全部还原后再次执行，最终 exit 1：30 步中 28 步通过、2 步环境失败，77 秒。Format、Clippy、Web lint/typecheck/build、相关 unit/kernel 用例通过；shell-live 第 3 条监听 127.0.0.1 报 `listen EPERM`，Web e2e 全局准备/清理报 `spawnSync ps EPERM`，浏览器用例未取得运行证据。最终日志 `.autome/output/r6-quick-final.txt`，首跑 `r6-quick.txt` 仅作诊断，最终判定以复跑为准。未修改权限和门禁脚本，由宿主复验这两步。

还原后的独立验收再次通过 73/73，日志 `.autome/output/r6-final-accept.txt`。`git diff --check` 和状态块/单一证据指针检查通过。M-05 标待审、implementation-round 为 6/65；未关闭任何里程碑。
