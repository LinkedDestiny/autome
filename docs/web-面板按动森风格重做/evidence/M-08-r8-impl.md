# M-08 · 实现 #8

结论：代码与可执行子集通过；浏览器验收被环境阻断，M-08 保持开放，reopen 不增加。implementation-round 为 8/65。没有运行全量门禁，没有启动下一会话。

## 修复范围

- 四个 Stepper：并行 1–5、设计轮与预算系数 1–u32 最大值、会话时长 5–u32 最大值；每次只写一项 `config.set_loop`。未授权、提交中和未改动时禁用操作，拒绝原文呈现。
- 项目逐项比较 resolved/global，显示「与全局相同」或「项目值」，恢复只发该键的 null。每个字段只在自身已保存值变化时重置草稿，避免保存一项清掉其它项草稿。
- `/loop?project=<id>&tab=project` 搬入全部 Onboarding、日志、编辑器、只读配置、违规信息和移除操作；项目/会话/配置事件触发刷新。旧地址 302；看板、新增成功及部分成功的入口同步改向。
- 内核仅更换 `set_loop` 两处校验的技能来源：全局 scan_global、项目 scan；不改变配置写入语义、账本或协议。
- 同页扩展样式检查发现起点已有 Loop 抽屉散写颜色、20px 圆角和不存在的令牌。换为现有 animal 令牌，原样式守卫未放宽。旧跳转路由继续保留实时刷新导出守卫。

## 命令、红绿与数量

原始输出均在 `.autome/output/`，这里仅记判据。

1. `cargo test -p automed --lib set_loop`
   - 修复前：2 个，1 通过、1 失败，新增 `set_loop_accepts_when_a_bound_skill_exists` 在已绑定技能后被拒；见 `m08-rust-red.txt`。
   - 修复后：2 个全部通过；见 `m08-rust-green.txt`。新增用例遍历全局和项目技能，且删除技能后仍拒绝写入并保留旧值，防止直接绕开校验。
   - 上一轮审计 M-07 已通过，无审计退回检查需要逐字搬入；上述是本里程碑主动建立的红绿回归。
2. `cargo build -p automed`：成功，真实内核测试使用更新后的二进制；见 `m08-build.txt`。
3. `(cd apps/web && npx vitest run test/loop-defaults.test.ts test/loop-real.test.ts test/settings-config.test.ts test/settings-onboarding.test.ts test/settings-real.test.ts test/settings-onboarding-real.test.ts test/w9-layout.test.ts test/add-project.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts test/loop-page.test.ts test/live-routes.test.ts test/styles.test.ts)`
   - 最终 14 文件、139 个全部通过；见 `m08-final-tests.txt`。其中验收指定 11 文件 96 个，直接相关扩展 3 文件 43 个。
   - 收尾调整路由 action 参数传递与移除无用组件导出后，重跑 `test/loop-defaults.test.ts test/settings-real.test.ts test/settings-onboarding.test.ts test/settings-config.test.ts test/testids.test.ts`：5 文件 49 个全部通过，见 `m08-last-edit.txt`；不重复计入总数。
   - 首次验收子集 94/95 通过，发现看板旧入口；修复后通过。扩展首次 39/43，通过补齐跳转路由刷新声明、修正起点的样式违规转绿。
4. `(cd apps/web && npx playwright test e2e/layout.spec.ts e2e/manual-items.spec.ts -g "项目设置|W9 H-01")`
   - 计划 2 条，实际执行 0 条。globalSetup 的 `leftoverProcesses` 调 `ps` 被沙箱拒绝：`spawnSync ps EPERM`；清理检查同样受阻。见 `m08-browser.txt`。未修改框架、未跳过检查、未取得截图或一屏通过证据。
   - 已在 layout「项目设置」中加入旧地址跳转断言和截图保存点，等待具备原测试所需权限的环境执行。

数量对照：本轮验收范围为 Rust 2 + Web 96 + 浏览器 2 = **100 个定义用例**，其中 **98 个通过、2 个环境阻断未启动**。按上一轮源码中相同选择范围计，基线是 Rust 1 + Web 84 + 浏览器 2 = 87，本轮 **+13**：Loop 默认值新文件 +8、loop-real +2、settings-real +1、add-project +1、Rust +1，未删旧用例。此同范围基线是源码核对，不冒充上一轮实跑。上一轮 M-07 实际记录为 1 + 78 + 2 = 81，与本轮验收定义 100 差 +19，其中 +13 为新增，另外 +6 为里程碑选集差异。直接相关扩展另有原有 43 个，所以本轮全部指定/相关子集共 **143 个定义、141 个通过、2 个环境阻断**；重复运行不累加。

## 标待审之前的自审清单

本轮虽保持开放，仍逐项执行自审，不以环境阻断省略可执行检查。

1. **情形表逐行**
   - E-6 四项范围：loop-defaults 的真实回包渲染、上下限、非法输入用例通过。
   - E-6 单项写入：四键分别断言请求仅包含当前键，全局无 project_id、项目带 project_id，通过。
   - E-6 来源文字：四项相等、单项不等均检查，未将相等称为继承，通过。
   - E-6 逐项恢复：四项 null 请求和真实读回通过；随后再改全局，项目继续继承，通过。
   - E-7 Onboarding、两份编辑器、日志、运行/完成/跳过、只读配置、错误提示、移除：原 settings 四组测试保留并改用新路由导出的处理器，通过；新路由完整 loader 和移除 action 分派也实跑通过。
   - E-7 旧地址 302、新增成功入口、项目 ID 编码和看板入口：通过。
   - E-7 1512×944 一屏：未执行，ps 权限阻断；不声称视觉验收通过。
2. **失败分支**
   - 未授权/忙碌：全部输入与按钮的禁用渲染检查通过。
   - 离线、授权拒绝、读盘失败、超时：Loop 默认值 action 的注入失败用例保留原文；真实坏 TOML 与既有设置读盘错误用例通过。
   - 空值/空白/未知键/全局 null/越界：不发送写入，通过。
   - 技能失效：Rust 删除原先存在的技能后拒绝且配置不变，通过；会话时长 4 的拒绝与文件不变亦通过。
   - 外部服务不可达：本功能无第三方服务，内核离线分支已检查；不适用外部网络验收。
3. **全部迁移**
   - 全局四项旧值→新值→读回；项目继承→覆盖→恢复→全局再变时继续继承，另一项目不受覆盖影响：真实内核通过。
   - 合法配置→非法请求→拒绝且保持旧值：Rust 通过；无网关→失败且无写入：unit 通过。
   - 旧设置 URL→302→新项目信息 loader；该页移除→根页：真实 handler 通过。
   - Onboarding 运行/正常结束/异常结束/重启、编辑/完成/跳过由保留的 onboarding 测试覆盖，通过。浏览器页签点击与动态禁用的视觉状态未实跑，已列为阻断。
4. **域边界**
   - 四键逐项最小值、最大值、下界减一、上界加一、小数、NaN、Infinity；空串、空白、前导零 02、未知键、null 作用域：通过。
   - u32 最大值 4294967295 对设计轮、预算系数、会话分钟有效，避免 JS 大值经内核 u32 转换截断。
   - 项目 ID 中 `&` 与 `=` 编码通过。闰日/跨年不适用：本里程碑无日期输入；单元素已由逐项单键请求覆盖。

## 指定快检与最终静态验证

- 已执行用户指定的 `sh scripts/ci-quick.sh --since 3dbd28ed5af9dde3a5a60d1649f615d61e5e8012`，见 `m08-quick.txt`。报告执行 92 步、沿用 0 步、失败 1 步，用时 146 秒；唯一失败是浏览器 globalSetup 的 `spawnSync ps EPERM`。超出 120 秒快检预算的步骤由脚本明确列为未跑，不计通过。快检自动选择的其它测试不重复累加到上面的固定子集数量。
- 快检包含 Rust fmt/clippy、Web lint/typecheck/build；收尾修改后再次独立核对 Web lint/typecheck，均退出 0；见 `m08-final-lint.txt`、`m08-final-types.txt`。本轮没有执行 `sh scripts/ci-local.sh`。
- 最后将 Stepper 的属性展开改为显式传参，避免 React 将字段 key 作为展开 key 的开发期警告；重跑 loop-defaults 8 个用例全部通过，见 `m08-stepper-final.txt`，不增加用例总数。

## 剩余工作

保持 M-08 开放，在允许现有进程扫描与浏览器启动的环境重跑上述浏览器验收并查看截图；全部验收通过后方可标待审。本次不计 reopen，环境问题不是协议失败。
