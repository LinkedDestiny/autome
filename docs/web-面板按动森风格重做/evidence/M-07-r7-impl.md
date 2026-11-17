# M-07 · 实现 #7

结论：实现已落地，里程碑保持开放。浏览器夹具在启动阶段被沙箱阻止（`spawnSync ps EPERM`），未取得完整验收通过证据，不能标待审。

## 改动

- `/loop` 读取真实配置、技能及项目列表；全局和项目范围切换；六种颜色角色卡、四张固定/人工虚线卡、轮数预算、内部滚动区。
- 编辑抽屉支持工具、模型、Effort、技能移除、启用草稿；任意 Effort 保留为只读「其它」。项目只发送变更字段，全局发送角色五字段；恢复时明确整角色清除确认。
- `role-rules.ts` 对齐内核三对模型纪律、停用免检、技能存在/可见性；按原始模型字符串比较（空白不自行归一化）。批量保存搜索合法中间态顺序，失败即停止，保存成功项清除草稿，未保存项保留。
- 内核拒绝 message 原文显示。停用文案依据 `autome-domain/src/task.rs` 的 `next_after_design`、角色 enabled 转移及 scheduler 的复盘跳过逻辑核对，符合设计，无需修改权威设计。
- 新 Rust 测试通过真实 dispatch 两次修改设计配置，验证两次后续会话 model/effort/skills 与提示词技能指令。
- 上轮审计 M-06 通过，无要求逐字搬入的缺陷判别检查。

## 命令与结果

原始输出均在 `.autome/output/`，本文件不转存。

1. `(cd apps/web && node test/fixtures/m07-capture.ts)`：成功。真实内核临时 HOME 捕获 `m07-global.json`、`m07-project.json`（含模型覆盖）、`m07-skills.json`；夹具路径仅为捕获时来源，不作为运行输入路径。
2. `cargo test -p automed --test end_to_end a_role_written_through_set_role_reaches_the_next_session`：1 个通过，0 失败/忽略；日志 `m07-rust.txt`。两个会话分别使用 m07-first/low 与 m07-second/medium，均含 m07-skill。
3. `(cd apps/web && npx vitest run test/role-rules.test.ts test/loop-page.test.ts test/loop-real.test.ts test/live-routes.test.ts test/labels.test.ts test/testids.test.ts test/test-groups.test.ts)`：7 文件、78 用例通过，0 失败/跳过；日志 `m07-vitest.txt`。
4. `(cd apps/web && npx playwright test e2e/layout.spec.ts e2e/controls.spec.ts -g "Loop 配置|E-1")`：夹具启动失败，2 条目标用例均未执行，不能记作通过或正常跳过；日志 `m07-browser.txt`。失败发生在 `leftoverProcesses` 执行 ps 时，未删除或绕过安全检查。没有截图和视觉验收结论。
5. `npm run typecheck --prefix apps/web`、`npm run lint --prefix apps/web`：初次实现检查通过；最终变更以指定快检记录为准。`cargo fmt --all`、`git diff --check` 通过。
6. 指定快检 `sh scripts/ci-quick.sh --since 14be40271b8da408abaa6c9f628c81d8e7a32414`：结果见下方最终记录。未运行 `scripts/ci-local.sh`。

## 用例总数与基线差

- 本轮验收子集：Rust 1 + Vitest 78 = **79 个实际执行通过**；另外浏览器 2 条被环境阻塞，完整目标为 81 个。
- 同一 Vitest 文件集合的起点基线按未改用例与新增清单推导为 43 个（未切换起点检出重跑），本轮 +35：role-rules 新增 15、loop-page 新增 13、loop-real 新增 6、testids 新增 1；live-routes/labels/test-groups 原有数量不变。Rust 新增 1，因此实际执行子集相对同范围基线 +36。loop-real 单个对照用例内部覆盖 17 行，不把行数冒算为 Vitest 用例数。
- 上轮 M-06 是 115 个 Vitest + 6 个浏览器 = 121；本轮实际执行数相差 -42，源于里程碑验收文件集合更换及本轮浏览器启动阻塞，不是删掉上轮用例。上轮用例未在本轮重复全跑。

## 标待审前自审清单（已逐项执行，可执行部分与阻塞分开）

1. **情形表逐行**：六卡顺序/颜色、四固定卡、CLI/Effort 默认、provenance、冲突两卡标红、项目稀疏字段、任意 Effort 和恢复文案均由 loop-page 检查；三对同/异模型、任一方停用、默认/空白、技能不存在/不可见、runtime 覆盖及保存顺序由 role-rules 检查；真实内核 17 行逐行一致。卡片外观、抽屉交互及一屏布局尚待两条浏览器用例。
2. **失败分支**：同模型/default 冲突、技能不存在/不可见由真实内核拒绝；action 拒绝原文及中途失败已保存项由 loop-real 验证；读盘坏 TOML、离线和未授权禁用由新增用例验证。空模型是合法 CLI 默认（需避免配对冲突），已覆盖。外部服务不可达/超时不适用：本页不调用外部模型服务；内核离线有显式错误。浏览器失败属于环境阻塞，未伪装成产品分支覆盖。
3. **全部迁移**：全局修改→读回、项目覆盖→另一项目不变→恢复全局、合法顺序/无合法顺序、逐项成功→后续失败→停止均有执行通过证据；开关两方向的规则变化、任一方停用在规则测试覆盖。实际点击编辑→草稿→关闭保留→重开、提交后草稿清理、范围切换、确认/取消恢复仍需浏览器交互复验，不以静态检查代替。
4. **输入域边界**：空串模型、空白模型精确匹配、null/任意字符串 Effort、零改动、单角色、技能单元素与不存在技能已覆盖；模型互换无合法顺序已覆盖。日期/闰日/跨年及数字上限不适用：M-07 编辑字段无日期数值域，默认值编辑属于 M-08。超长输入沿用 write-gate 限制，本轮未扩大其边界或声称重测全套网关。

## 后续入口

保持 M-07 开放、reopen=0。具备 ps/浏览器权限的下一轮先跑上述两条浏览器验收，补齐自审第 3 项交互及截图核对；完整验收通过后才标待审，不进入 M-08。

## 最终快检记录

指定快检重跑结果：退出码 1，执行 70 步、失败 1 步，唯一失败项为 Web e2e（同一 `spawnSync ps EPERM` 环境限制）；Rust fmt/Clippy、Web lint/typecheck 均通过。日志 `.autome/output/m07-quick-final.txt`。超出脚本 120 秒预算的关联测试由脚本列为未跑，未算入本轮验收子集通过数。

首轮快检另抓到新增测试 JSON import 缺少 `with { type: 'json' }` 与离线服务夹具缺少 `gateway: null`，已修复；第二轮 typecheck 通过。最终验收 Vitest 子集仍为 78/78，通过数没有因修正类型而变化。
