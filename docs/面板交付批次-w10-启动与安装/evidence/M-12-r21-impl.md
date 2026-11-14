# M-12 · 实现 #21

结论：验收通过，申请待审。本轮只推进 M-12，上一轮审计 #19 对 M-11 结论通过，无退回检查需要迁移。

## 实现

新增 POSIX sh 脚本 prepare-web：缺依赖目录、缺指纹或锁文件变化时 npm ci，成功后写入纯 SHA-256 摘要加换行，与 ci-local.sh 格式互认；无论是否安装都执行 build。安装与构建错误原码退出。package.sh 在 stage-web 前调用它。

## 命令与结果

- 运行设计 M-12 验收命令全文（唯一调整为 mktemp -d "$PWD/.autome/output/m12-clean.XXXXXX"，使临时产物位于约定目录），退出 0。原始输出：`.autome/output/m12-acceptance.txt`。
- `(cd apps/web && npx vitest run test/prepare-web.test.ts)`：5/5；`(cd apps/desktop && node --test test/packaging.test.js)`：7/7；失败 0，跳过 0。
- `sh -n scripts/prepare-web.sh scripts/package.sh`：退出 0。
- git ls-files -co --exclude-standard apps/web 经 tar 复制到干净目录；确认无 build，再真实 npm ci、npm run build、node scripts/stage-panel.ts；确认暂存产物 bin/autome-web.js 与 build/server/index.js 均存在。真实安装构建暂存通过，无网络阻塞。
- 本轮范围共 12 条自动测试，相对上轮同范围基线 6 条增加 6（prepare-web 新增 5，packaging 新增 1）；干净目录暂存另计 1 次集成验收，不虚计成测试用例。上一轮 M-11 的 53 条 vitest 和 12 条 layout 属于另一范围，本轮未重跑，不能当成减少 53 条。
- `npx eslint test/prepare-web.test.ts`：退出 0；`git diff --check`：通过。未跑全量门禁或宿主快检。
- 开发首跑遇到测试夹具模板字符串转义错误（0 用例），修正后全部 5 条执行通过，未隐藏失败。

## 标待审之前的自审清单

1. **情形表逐行**：无 node_modules → ci/build/正确指纹（第 1 条）；指纹相符 → 仅 build（第 2 条）；旧依赖缺指纹以及锁文件变化 → 重装并更新指纹（第 3 条）；安装失败 → 原码 23、无 build/无成功指纹（第 4 条）；构建失败 → 原码 29、保留指纹、下一次只 build（第 5 条）。打包 prepare → stage → electron-builder 顺序由新增 packaging 检查通过；真实干净目录暂存通过。
2. **失败分支**：设计点名 npm ci 与 build 失败均注入实跑，原退出码与后续行为见第 4/5 条。授权、监听超时与用户输入不属于本脚本契约，不适用；网络安装已真实通过。
3. **全部迁移**：未安装→已安装→构建成功；指纹匹配→构建；缺指纹/锁变化→重装→构建；安装失败→退出；已安装→构建失败→保留安装状态→重试成功，均由上述 5 条覆盖。
4. **域边界**：全部测试在含中文及空格的 web 路径运行，正确 cwd/argv 已断言；无目录、无指纹、匹配/不匹配指纹均覆盖。缺省路径实现锚定脚本所在仓库 apps/web，package 调用不传参；显式路径通过真实干净目录验收。日期、数值最大值等输入域不适用，本脚本没有这类参数。

## 负向对照与恢复

- 原 package.sh 未调用 prepare 时新增 packaging 检查红（6 通过、1 失败），加入调用后 7 绿：`.autome/output/m12-red.txt`、`m12-acceptance.txt`。
- 临时注入「有 node_modules 就跳过 build」「忽略指纹」「安装失败仍继续」三个变体，prepare-web 测试分别退出 1；日志 `.autome/output/m12-negative-{skip-build,ignore-hash,ignore-install-failure}.txt`。均用 finally 恢复原文件，恢复后 5/5 通过，见 `.autome/output/m12-restored.txt`。

无阻塞。提交使用 NO_CI=1，避免自行启动项目门禁；后续由 Autome 调度审计。
