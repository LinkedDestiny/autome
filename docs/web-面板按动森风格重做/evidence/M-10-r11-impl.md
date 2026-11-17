# M-10 · 实现 #11 · 开放（浏览器验收受阻）

## 改动与范围

环境页采用丝带、四列工具行、版本胶囊和独立绿色驱动卡；内核路径与构建标识通过与 `/api/instance` 共用的 instanceStatus 服务读取，失败明确回退。检测、启动、检查时间用 ClockProvider 的相对时间和 time 的绝对时间 title。新建任务改用 Segmented、Check、多行 inp 和可聚焦的隐藏文件输入；添加项目使用 Input、Check 和 inp 下拉，保留原生目录选择逻辑、字段、校验及 action。

仅推进 M-10。上一轮 M-09 审计通过，没有需要逐字移植的退回检查。未改协议、未运行全量门禁、未启动下一会话。

## 验收命令与用例数

- `(cd apps/web && npx vitest run test/env-page.test.ts test/env-ops.test.ts test/env-real.test.ts test/clock.test.ts test/task-new.test.ts test/add-project.test.ts test/w9-layout.test.ts test/labels.test.ts test/testids.test.ts test/styles.test.ts)`：10 文件、107 用例全部通过。最终输出 `.autome/output/m10-unit-final.txt`。
- 同范围起点基线为 100 个，本轮新增 7 个：env-page +4、clock +1、task-new +1、add-project +1。设计下限 99 不是当前基线。上一轮 M-09 的 145 个 Vitest 用例属另一验收集合，与本轮 107 相差 -38（集合切换），不能作为删减用例的判断。
- `(cd apps/web && npx playwright test e2e/layout.spec.ts e2e/manual-items.spec.ts -g "环境|新建任务|添加项目补全|W9 H-02|W9 H-04")`：应执行 5 条，实际执行 0 条。夹具初始化的进程检查 `spawnSync ps EPERM` 被沙箱拒绝，清理也报告同一拒绝；没有启动浏览器或生成截图。输出 `.autome/output/m10-browser.txt`。本轮验收总集合为 112 个（107 Vitest + 5 浏览器），实际完成 107 个；浏览器五条未执行，不能记为通过。
- 首次组件验证发现新 CSS 令牌名错误和新增内部滚动区未列入测试白名单，已分别修复为现有令牌及明确的 env-components 滚动区；原有断言保留。

## 标待审之前的四项自审（本轮未标待审）

1. **情形表逐行**：G-1 页头问题数/全正常摘要、工具四列结构、内核信息源与失败回退、绿色驱动卡和所有时间由 env-page 12 条覆盖；G-2 类型分段、正文、文件输入、依赖字段及勾选、添加路径/工作区/文档仓库/跳过字段由 task-new 12 条、add-project 17 条覆盖。布局 CSS/令牌由 w9-layout、styles、testids 通过。真实页面一屏与视觉检查未执行，受上述浏览器夹具阻塞。
2. **失败分支**：env-page 检查拒绝授权、读盘失败、超时、内核离线、recipe 失败、写操作失败、终端不可用；env-ops 覆盖服务不可达、前置条件不满足、无登录命令、额外参数拒绝和非 macOS 降级；add-project 覆盖目录拒绝与部分成功，task-new 覆盖空/无效正文、无项目与上传描述缺失。全部通过。剪贴板和原生选择器的真实 UI 验收受浏览器阻塞，未宣称实跑。
3. **全部迁移**：env-ops 第 7 条覆盖首份报告、probing false→true→false、generation 增加和重复报告；env-real 第 3 条验证实际强制检测事件和 generation 更新；env-page 覆盖安装/登录成功、失败及终端降级。task-new 现有类型切换测试覆盖 feature/bug 双向和编辑正文保留；add-project 覆盖探测、可确认、拒绝、成功与部分失败。控件替换未增加业务状态机，浏览器交互迁移仍待实跑。
4. **输入域边界**：clock 覆盖零、秒/分/时/日界限、未来、空串、闰日及跨年；task-new 覆盖正文/附件限制和依赖 0/1/8/9；env-ops 覆盖空、未知、尾空格、非字符串与超长组件名；add-project 覆盖补全截断、请求取消、路径更改后不得确认。均通过。前导零不适用：本轮无新的数字解析输入。

## 负向对照与还原

- 把驱动启动时间改回直接输出 started_at：`npx vitest run test/env-page.test.ts` 退出 1，新时间用例失败，输出 `.autome/output/m10-negative-time.txt`。
- 把 Segmented 的 name 从 kind 改成 type：`npx vitest run test/task-new.test.ts` 退出 1，新字段用例失败，输出 `.autome/output/m10-negative-kind.txt`。
- 两个注入均已原样还原，再次运行整个 M-10 Vitest 集合，107 个全绿。

## 结论

实现与静态/内核子集已就绪；浏览器五条未获通过证据，M-10 保持开放、reopen 不增加。下一轮应在允许进程检查的环境运行原验收命令，检查截图、一屏与交互，成功后才标待审。H-05 的人工原型对照保留原清单。

## 指定快检与关联回归

- 已运行指定命令 `sh scripts/ci-quick.sh --since 82aaa4a4d8221d2e4ddec93717a9d3f8c11df7dc`，原始输出 `.autome/output/m10-quick.txt`。Format、Clippy、Web lint、Web typecheck 通过。
- 快检发现 add-project-real 两条仍假定 M-08 之前的 /projects/:id/settings 跳转，第二条从旧路径拆项目 ID 导致空 ID。按已批准 M-08 的 /loop?project=…&tab=project 契约更新 URL 断言与解析，保留注册/成员/看板断言，生产行为未改。该检查红证据在快检日志；修复后 `(cd apps/web && npx vitest run test/add-project-real.test.ts)` 4 个全过，见 `.autome/output/m10-add-project-real.txt`。
- 本轮明确选择的验收与关联回归共 111 个实际通过（107 + 4），加未执行的浏览器 5 条为 116；同范围起点 109（100 + 4 + 5），增量仍为 7。快检额外调度的用例与这一集合重叠，不重复累加。
- 快检还发现未改动的 board-detail-real 第 1 条期待“门禁未跑过”，当前 active.gate 为 null；该文件与 task-card-model 本轮均未修改。记录为快检未通过项，本轮不扩展看板实现范围。
- 快检最终执行 56 步，失败 3 步，耗时 172 秒：添加项目旧断言（上述已单独修复并复跑通过）、看板旧断言、浏览器 ps EPERM。超出快检 120 秒准入预算后的测试未运行，清单见原始日志；不能宣称快检通过。Web build 同样通过。
- `git diff --check` 与状态块字段/证据指针检查通过。由于浏览器和未修改的看板检查仍阻塞，本轮未再次重跑整个快检；关联改动已有修复后的 4 条绿证据。
