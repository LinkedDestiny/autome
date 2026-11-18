# M-10 · 实现 #17

## 修改与范围

落实 E-Q1～E-Q3：纯函数 toolOk 同时检查安装、登录和问题列表；正常工具说明默认收起，异常默认展开，内核行也标记健康状态。使用原生非受控 details，稳定 key 与布尔 open，不增加同步 effect；正常行路径与状态并排，完整路径保留 title。添加页主体采用纵向 flex，候选列表分配剩余空间并允许收缩，成员列表保留 160px 上限。四列共用列体 flex: 1 1 0。

更新 layout.spec.ts 的环境初态/手动切换、候选列表高度、最后一张取消卡片与列体/视口边界断言。上一审计 M-09 通过，没有退回检查需要搬运。

## 命令与结果

- 验收命令：`(cd apps/web && npx vitest run test/env-page.test.ts test/add-project.test.ts test/board.test.ts test/board-cancelled.test.ts test/styles.test.ts test/w9-layout.test.ts test/labels.test.ts)`。
- 起点同命令：7 文件、84 用例通过（`.autome/output/m10-baseline.txt`）；最终：7 文件、88 用例通过，0 失败、0 跳过（`.autome/output/m10-final.txt`）。差值 +4：环境正常折叠 1、16 种异常/正常组合与空集合边界 1、四种健康状态迁移 SSR 属性 1、候选列表伸缩规则 1；原有看板守卫加强，不改变用例数。上一轮 M-09 的 55 个属于不同子集，不作本轮同口径基线。
- 三个设计负向对照分别临时写死 open=true、恢复候选 max-height:160px、删除列体 flex，分别运行 env-page、add-project、styles 对应文件，全部退出 1；原文均在 finally 还原，最终验收全绿。日志 `.autome/output/m10-negative-{open,height,board}.txt`。
- 中间一次新增测试的正则字面量写错导致解析失败，修正后全绿；不把解析错误算作产品回归红证据。
- 浏览器子集：`(cd apps/web && npx playwright test e2e/layout.spec.ts --grep '1512×944 (环境|添加项目补全|已取消展开)$')`。启动失败 `spawnSync ps EPERM`，0 个页面用例执行；日志 `.autome/output/m10-browser.txt`。实际几何、手动切换与重检测保留行为未据此宣称通过，留宿主浏览器环境复验；H-06 保留。
- 未运行全量门禁 ci-local.sh。

## 标待审之前的自审清单

1. **情形表逐行**：正常 present + ok/not_applicable + 不在 problems → 收起；未安装、expired、unknown、在 problems → 展开，16 种组合逐项通过。五行健康标记和四个说明 summary 通过。添加页剩余高度、候选伸缩、去掉上限、成员上限分别由新增 CSS 守卫与既有 w9 守卫通过；看板四列共用规则及取消分组由 board/board-cancelled/styles 通过。浏览器几何断言已补，运行受上述环境阻断。
2. **失败分支**：验收里的 env-page 已运行拒绝授权、读盘失败、超时、内核未连接、安装前置不满足、终端不可用、未知 intent 与离线写禁用检查，均通过；add-project 既有路径错误与操作失败分支通过。本轮不改变这些接口。
3. **全部迁移**：健康→健康、健康→异常、异常→健康、异常→异常四组 SSR 前后 open 属性均通过。保留相同 key、非受控 details 与 open={!ok}，无 effect 强制覆盖用户状态。手动展开/收起及同状态重检测后的实际 DOM 保留依赖 React 差异更新，浏览器实跑受阻，明确不把 SSR 等同交互验收。
4. **域边界**：空工具集合、单工具、完整四工具加内核，present 两值 × login 四值 × 问题列表命中两值全部覆盖；路径缺省值由原 fixture 和既有用例覆盖。数字日期等输入域不适用：本轮无新增此类输入。候选空列表和成员列表已有用例通过；真实容器的像素边界待宿主浏览器复验。

## 指定快检与提交

- 已运行 `sh scripts/ci-quick.sh --since 9f19bf8f73b2123e354427ec3d03eb437fe01e8c`。初次与子集浏览器构建并发，出现构建 manifest 缺失及其后续模块加载失败；固定最终代码后串行复跑，构建和 load-context 恢复通过。最终日志 `.autome/output/m10-quick-final.txt`：27 步，2 步失败，74 秒；shell-live 监听 127.0.0.1 被 EPERM 拒绝、Web e2e 执行 ps 被 EPERM 拒绝。静态检查通过，但快检整体未通过，不以脚本退出码 0 代替其失败摘要。
- `git diff --check`、状态块字段与里程碑数量检查通过；仅 M-10 改为待审，9 个已完成里程碑保持不变。
