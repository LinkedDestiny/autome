# M-10 实现 #19：快检类型错误修复

结论：本轮是修复轮，不推进里程碑。M-10 原为开放，保持开放；其余里程碑状态不变，implementation-round 为 19/65。原快检通过，最终以 Autome 提交后重跑为准。未运行全量门禁。

## 根因与修复

宿主 af17897fdb 快检唯一失败项是 Web typecheck。测试夹具 taskArgs 缺少 ShouldRevalidateFunctionArgs 必填的 nextParams，类型断言产生 TS2352。补上与同 URL 对应的 nextParams，并将强制断言改成显式类型注解，让项目现有 tsc 持续检查完整夹具，不用 unknown 绕过检查。生产逻辑及行为断言不变。

同类核对：该文件仅这一个 ShouldRevalidateFunctionArgs 夹具；另两处旧 as never 夹具不属于本次报错，未扩大修改。上一审计 #11 通过 M-09，没有缺陷判别脚本需要搬入；此次失败检查已经是项目 package.json 的 typecheck，直接原样复跑形成红绿回归。

## 命令与结果

原始输出保存在 .autome/output/，此处只记结果。

1. 修复前 `npm run typecheck --prefix apps/web`：tsc 报 TS2352，定位 test/live-routes.test.ts:4，缺少 nextParams（r19-typecheck-red.txt）。外层读取日志的 tail 返回 0 不代表类型检查通过。
2. 修复后同一命令：退出 0（r19-typecheck-green.txt）。共 1 项静态检查，错误数 1 → 0，不计为行为测试用例。
3. 原样运行用户要求的快检命令：

```sh
if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi
```

结果退出 0，5 步通过、0 失败、0 沿用，19 秒；包括 Format、Clippy、Web install、Web lint、Web typecheck（r19-quick.txt）。脚本明确提示「未提供本轮起点，未跑改动相关的测试」，因此另外执行下一项，不把本次快检称为宿主 related 266 例全通过。

4. `(cd apps/web && npx vitest run test/live-routes.test.ts test/test-groups.test.ts)`：2 文件、总数 18，18 通过、0 失败、0 跳过（r19-unit.txt）。与上一轮同范围实测基线 18 相差 0；本轮没有新增、删除或跳过用例。与宿主 related 范围 266 不同，不能直接比较为减少 248 例。
5. `git diff --check`：通过。没有重跑 BL-01 浏览器用例；上一轮未完成的浏览器验收仍待复验，不标待审。

## 四项自审

1. 情形表逐行：G.1 的本任务原事件、前置 task/merge 事件命中、无关任务与前置其它事件不命中，由 live-routes 的既有 14 例覆盖并通过。五秒浏览器显示项本轮不执行，仍未验收。
2. 失败分支：类型夹具修复不新增授权、读盘、外部服务失败分支，不适用。既有 loader 拒绝后恢复用例通过；原类型缺字段分支已红绿验证。
3. 全部迁移：登记、替换 after、清空依赖、卸载清除、跨任务隔离在既有子集中通过；修复本身不新增状态迁移。浏览器组件生命周期未在本轮实跑。
4. 输入域边界：空依赖、单/多前置、空 subject、空 kind、无关 id 在既有子集中通过。日期与数字极值不适用，本轮不改变用户输入域。

## 隔离与交接

只修改测试夹具与本任务记录；无生产逻辑改动、无新服务、未接触真实账本，不切分支、不推送、不启动下一会话。后续先由 Autome 重跑快检，再在允许的环境完成 M-10 原浏览器验收。
