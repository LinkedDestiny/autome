# M-10 实现 #18

结论：实现与可运行单测已通过；浏览器验收被沙箱阻塞，保持开放，未标待审。上一审计 #11 通过 M-09，没有退回 M-10 的检查需要搬入。

## 修复

- live-routes 增加按任务 id 登记相关 subject 的表；自己的事件维持原前缀，前置只接受 task.* / merge.*。
- 任务页 effect 登记 loader 的 after，依赖或任务变化时清理再登记，卸载清理；没有按任务是否开始过滤。
- 新增三个路由回归用例，直接调用生产任务页 shouldRevalidate。BL-01 浏览器用例使用 3000ms live 实例及临时 HOME/AUTOME_HOME、当前 checkout 内核；批准前置后五秒内依赖区更新、详情数据重验、子任务仍排队且无整页重载。实现夹具停在实现轮，避免很快推进到审计造成状态断言漂移。
- 浏览器同时检查详情数据请求包含任务路由，避免仅 root 的 shell 状态更新掩盖详情未重验。

## 命令与结果

原始输出均在 .autome/output/，未写入本文。

1. 改动前：(cd apps/web && npx vitest run test/live-routes.test.ts test/test-groups.test.ts)：15 个全过（11+4），m10-baseline.txt。
2. cargo build -p automed -p autome-cli：退出 0，m10-cargo.txt。
3. (cd apps/web && npx vitest run test/live-routes.test.ts test/test-groups.test.ts)：18 个全过（14+4），m10-unit.txt；还原负向变异后再次 18 个全过，m10-green.txt。与实测基线差 +3，全来自上述路由用例。
4. npm run build --prefix apps/web：退出 0，m10-build.txt。
5. (cd apps/web && npx playwright test e2e/controls.spec.ts -g "BL-01")：启动失败，spawnSync ps EPERM，m10-browser.txt；新增 1 个用例（筛选基线 0 → 1），实际执行 0，不算通过或主动跳过。用例最终补充的夹具保持与请求断言尚未浏览器实跑。
6. (cd apps/web && npx vitest related --run app/lib/live-routes.ts 'app/routes/tasks.$id.tsx')：39 个文件，37 通过、2 失败；266 个用例，262 通过、1 失败、3 因 beforeAll 失败未执行。shell-live 的监听用例与 task-live-real 的启动遇 listen EPERM，后者清理另报 ps EPERM。日志 m10-related.txt。相同选择范围新增 +3；按当前结果扣除新增数对应 263，但未重跑旧版本的 related 集合，不把推算称作上一轮实测基线。
7. 负向对照：暂把任务页匹配器恢复 bySubjectParam，路由测试退出 1（m10-negative-own-only.txt）；暂删 relatedSubjects.delete(key)，路由测试退出 1（m10-negative-no-cleanup.txt）。每次 finally 还原源码；再跑第 3 项绿。浏览器负向对照因相同环境阻塞未跑。
8. 仅对四个改动 TS/TSX 文件运行 npx eslint --max-warnings 0：退出 0（m10-eslint.txt），不是项目全量 lint。
9. git diff --check：通过。未运行 ci-local、ci-quick 或宿主快检组合。

本轮自审与用例计数仅基于上述子集，不能代表全量通过。两个 Vitest 集合有交集，不相加冒充独立用例总数。

## 标待审之前的四项自审（已执行，但因验收阻塞未标待审）

1. 情形表逐行：G.1 本任务原前缀事件命中、前置 task.updated 与 merge.completed 命中、无关任务不命中、前置 session.finished 不命中，均由新增用例通过。详情 effect 确认使用 data.task.after，不受任务阶段限制。五秒浏览器项未执行。
2. 失败分支：G.1 未点名授权、读盘、外部服务、超时等新增失败处理，不适用；既有 loader 失败后的重验恢复仍由 live-routes 原有用例通过。数据缺失时 effect 不登记并执行上次清理，源码核对。环境拒绝如上记录，未添加跳过。
3. 全部迁移：未登记→登记、旧前置→新前置、有依赖→空列表、登记→清理、另一任务登记不受清理影响均测试通过。真实组件卸载及 after 导航更新的 effect 接线经源码核对，未声称浏览器生命周期验收通过。
4. 输入域边界：无新增用户输入解析域；空依赖、单元素、多元素、空 subject、空 kind、无关 id 均测试通过。日期、数值最大值等不适用（本轮只消费内核 task id，不改变依赖表单验证）。

## 隔离与后续

浏览器 setup 在 leftoverProcesses(root) 失败，尚未 prepare 或 spawn 服务；task-live-real 在端口预探测失败，尚未起内核。相关测试其余清理由原夹具执行；因 ps 被禁无法独立完成全机无残留核对，不声称已验证。未接触真实账本、launchd 或真实面板；无手工启动服务。截图视觉检查未执行（浏览器未启动）。

下一步：在可执行 ps、可监听 localhost 的宿主复验 M-10 完整验收命令与受阻 related 子集，成功后才标待审。本轮不增加 reopen，不推进 M-11。
