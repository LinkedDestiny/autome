# M-10 · 实现 #18 · 宿主快检修复

## 根因与修改

本轮不推进新里程碑，M-10 保持待审。设计 E-Q1 要求正常工具的安装说明默认收起，旧 W9 H-04 直接要求其中的复制按钮可见，遗漏展开步骤。宿主在起点提交运行 35 个 e2e：34 通过、1 失败，明确为 env-copy 存在但 hidden，非命令或剪贴板错误。

在项目现有 `apps/web/e2e/manual-items.spec.ts` 的原用例内补用户操作：找到按钮所属 details，收起时先断言按钮隐藏，再点击 summary，确认 open，然后执行原有可见性、非空命令、页面原文一致、真实剪贴板读回断言。已展开分支不点击，以免反而折叠。不修改产品默认折叠设计，不强制点击或直接改 DOM，不 mock 剪贴板。检索 e2e 中 env-copy 的直接使用仅此一处；layout 的环境用例已有 summary 切换。

## 命令与用例数

- 原样复现命令：`(cd apps/web && npx playwright test e2e/manual-items.spec.ts --grep 'W9 H-04')`。改前日志 `.autome/output/m10-r18-red.txt`；初始化 `spawnSync ps EPERM`，0 个用例执行。本地未得到断言红；红证据来自用户提供的宿主失败记录。检查原已在项目测试体系中，无外置审计检查需要搬入，原四项复制断言全部保留。
- 修后同命令日志 `.autome/output/m10-r18-browser.txt`：同一初始化阻断，0 个用例执行，不能宣称浏览器绿。待宿主在可运行浏览器的环境重跑。
- M-10 验收命令：`(cd apps/web && npx vitest run test/env-page.test.ts test/add-project.test.ts test/board.test.ts test/board-cancelled.test.ts test/styles.test.ts test/w9-layout.test.ts test/labels.test.ts)`。7 文件、88 用例全部通过，0 失败、0 跳过，日志 `.autome/output/m10-r18-accept.txt`。较上一轮同口径 88 个差值 0；本轮加强既有 e2e 的操作步骤，不新增用例。浏览器套件仍为 35 个，本轮定向选择其中 1 个，实际执行 0，不能把未执行当通过。
- 未运行 `sh scripts/ci-local.sh`。

## 四项自审清单

1. **情形表逐行**：E-Q1 正常收起、异常展开由 env-page 验收通过；E-Q2 候选伸缩由 add-project/w9-layout 通过；E-Q3 列体伸展及取消分组由 board/board-cancelled/styles 通过。复制用例补正常收起→用户展开；实际点击与剪贴板检查因浏览器初始化权限受阻，保留宿主复验。
2. **失败分支**：验收子集中的拒绝授权、读盘失败、超时、内核离线、安装前置不足、终端不可用及路径操作失败检查通过。本轮不改这些产品分支。浏览器环境失败明确记录，未绕过检查。
3. **全部迁移**：验收子集的健康→健康、健康→异常、异常→健康、异常→异常属性检查通过。修复用例支持收起→摘要点击→展开，以及原本展开→保持展开；真实 DOM 迁移与复制结果本地未实跑，不以静态检查替代。
4. **域边界**：验收子集包含空工具集合、单工具、四工具及内核、安装/登录/问题列表组合、候选空列表；全部通过。本轮无新输入域，日期/数字边界不适用。命令非空与页面内容逐字一致的原断言保留，浏览器验证待宿主。

## 指定快检与提交

- 已执行 `sh scripts/ci-quick.sh --since a7544ed0439d8fe13b4ee325115b01fab26e41dd`，日志 `.autome/output/m10-r18-quick.txt`。摘要为 12 步、失败 1 步（Web e2e 初始化 ps EPERM）、28 秒；无匹配的 vitest related，本轮已另跑上述 88 个验收用例。静态检查与构建通过，快检整体未通过；脚本虽然返回 0，仍以失败摘要为准，不宣称绿。最终快检结果以 Autome 提交后重跑为准。
- `git diff --check` 通过；implementation-round 为 18/66，M-10 仍待审，其余里程碑状态不变。
