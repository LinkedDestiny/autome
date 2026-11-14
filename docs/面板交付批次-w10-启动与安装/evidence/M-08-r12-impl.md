# M-08 · 实现轮 12/65

## 实现与范围

按 C5 删除 renderer、7 个旧 src 模块、旧 UI/IPC/sidecar 测试及 screenshots 工具。打包只收 main/preload/src，资源为 core/autome 与 web，移除 AppleEvents 权限和用途说明。package.sh 构建 autome-cli、暂存 Web 后打包；本地与 GitHub Bundle 步骤检查 autome --version 并执行打包冒烟。README Layout/Packaging 同步。

样式断言在删除前从旧 renderer 冻结 token 字面量和字体/许可 SHA256，随后验证 Web 值一致。打包 smoke 的相对 AUTOME_SMOKE_APP 以 npm INIT_CWD 解析，避免 npm 切到 apps/desktop 后重复拼路径。

上一轮 M-07 审计通过，无退回检查需要新搬运。保留已经入库的 shell-smoke 4 条、shell-ledger 1 条回归，因此 Desktop 实际为 22 条而非设计早期的 17 条；设计就地修正。shell-ledger 仍实际执行临时 home 的 autome status，CI 将旧 automed 构建替换为 autome-cli，不能为追求纯测试删掉该回归。干净 Web 构建仍按既定范围由 M-12 处理。

## 命令、结果与基线

原始输出均在 .autome/output/，未运行项目全量门禁或宿主快检。

- `npm --prefix apps/desktop test`：22/22，通过、跳过 0；m08-desktop.txt。
- `(cd apps/web && npx vitest run test/styles.test.ts test/ci-config.test.ts)`：21/21，通过、跳过 0（styles 12、ci-config 9）；m08-web.txt。
- `test ! -e apps/desktop/renderer`、`sh -n scripts/package.sh scripts/ci-local.sh`、修改的两个 TS 测试文件 eslint、`git diff --check`：通过。
- `sh scripts/package.sh`：release 编译成功，core/autome 和 Web 启动器/build 暂存存在；安装 electron-builder 时 npm ENOTFOUND registry.npmjs.org，未生成 .app。m08-package.txt。
- 完整 M-08 验收链再次执行，设置 npm_config_fetch_retries=0 仅减少网络失败重试；前 43 条及删除检查通过，在同一安装步骤失败。m08-acceptance.txt。因 && 短路，打包冒烟 4 条未执行，不能声明 .app 已启动或无残留证明通过。M-08 验收契约明确允许此安装阻塞交审计轮或 GitHub package job 复验。
- 本轮验收声明用例总数 **47 = 22 + 21 + 4**，实际执行 **43**，通过 43，失败 0，跳过 0，另 4 条因无打包产物未执行；构建命令失败单独记账。上一轮基线 **23 = 16 + 3 + 4**，差 **+24 = packaging 6 + styles/ci-config 21 - 本轮不跑的 native-shell 3**。同口径 Desktop 新增 packaging 6；ci-config 从 8 到 9（+1）；styles 12、smoke 4 不变。旧 renderer 测试按 C5 删除，不把不同范围的总数误称为全库覆盖增加。

## 标待审之前的四项自审清单

1. **情形表逐行（C5）**：renderer/7 模块删除；旧测试与 fixtures/dom-harness 删除；screenshots/shots 删除；main/preload 保留 M-07 实现；packaging 重写 6 条通过；entitlements 保留 JIT/框架项、AppleEvents 删除；styles 冻结值 12 条通过；electron-locate/probe-sqlite 保留；本地 Desktop 命令保留；GitHub Desktop 去掉 automed、保留账本回归所需 CLI 编译。资源 core/web 均已暂存，打包产物待外部复验。
2. **失败分支**：shell-main 第 2 条实际检查读盘失败、外部不可达、空回包、非法地址、35 秒超时；第 4 条检查停止失败；第 6 条拒绝异源/空来源与取消目录选择。打包缺网络分支实际失败并停止，没有伪造成功。负向注入三项：恢复旧 renderer 路径、漏 web 资源、复制旧 automed 路径，各 exit 1；逐次还原后 43 条全绿，m08-negative.txt。漏 web 当前由 packaging 契约检出，未声称完成真实 .app 缺资源注入。
3. **全部迁移**：shell-main 覆盖启动→加载，自有→正常停止，自有+门禁→后台保留，复用→退出不停止，继任 instance→保留；IPC 两通道同源/异源、选择/取消、通知→点击聚焦导航、同源/异源导航、新窗口拒绝均通过。打包冒烟中的启动、复用、旧账本横幅、进程清理 4 条未执行，原因同上。
4. **域边界**：shell-main 通知 title 200/201、body 1000/1001、href 500/501、空串/null、协议相对和反斜线异源路径均通过；冻结 token 与两个文件 hash 精确一致。M-08 本身无新增数值或日期输入域，日期/闰日边界不适用。

## 结论

待审；审计轮需在可联网环境完成 package.sh，并执行打包冒烟 4 条，真实截图仍为 H-03。实现轮仅推进 M-08，未关闭里程碑；无进程服务由本轮启动（账本回归子进程同步退出，未启动 Electron）。
