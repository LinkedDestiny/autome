# M-07 · 实现 #13

结论：旧截图替换完成，M-07 待审。implementation-round: 13/65。本轮只推进 M-07。

## 修复与来源

审计 #8 发现 docs/images 三图仍为 1200 宽旧图。判别检查已逐字存在于项目测试 `apps/web/test/readme-images.test.ts` 的「三张图片都是 1512×944 PNG」，不复制第二份、不改断言。本轮原样运行审计失败命令，先红后绿。

本轮执行截图脚本仍被沙箱 `spawnSync ps EPERM` 拒绝，发生在任何服务、内核和浏览器启动之前。采用上一审计实际运行同一脚本生成、仍留在 `.autome/output/shots-2/` 的三个原始 PNG，逐文件复制到 `docs/images/`；没有缩放、改绘或伪造。来源运行及残留 0 证据见 `evidence/M-07-r12-audit.md`；该生成结果是上一审计证据，不能称本轮脚本运行成功。复制后三组 SHA-256 逐对一致，记录于 `.autome/output/m07-r13-sha.txt`。

## 命令与结果

- 原样 `(cd apps/web && npx vitest run test/readme-images.test.ts test/test-groups.test.ts)`：替换前 7 例，6 通过、1 失败，尺寸收到 1200×451；替换后 7/7 通过。日志 `m07-r13-red.txt`、`m07-r13-green.txt`。
- `cargo test -p autome-cli --test cli web_`：10/10；`cargo test -p autome-cli --bin autome`：67/67。`cargo build -p automed -p autome-cli && npm run build --prefix apps/web` 成功。日志 `m07-r13-accept.txt`。
- `node apps/web/scripts/screenshots.ts --out docs/images`、`node apps/web/scripts/screenshots.ts --out .autome/output/shots-1`、`node apps/web/scripts/screenshots.ts --out .autome/output/shots-2`：本轮均退出 1，ps EPERM；未绕过进程安全预检。日志分别为 `m07-r13-generate.txt`、`m07-r13-shots1.txt`、`m07-r13-shots2.txt`。两次可重复生成与无残留的成功证据仅引用上一审计，本轮留待审计环境复验。
- 空输出参数检查 `node apps/web/scripts/screenshots.ts --out ''`：正确拒绝，打印用法。日志 `m07-r13-empty.txt`。
- 临时移开 Web build（EXIT trap 原位还原），运行同一截图命令：正确报缺少构建并提示构建命令，未启动进程。日志 `m07-r13-missing-build.txt`；build 已恢复。

所有原始日志均在 `.autome/output/`。未运行 ci-local 或 ci-quick，交宿主执行。

## 标待审前自审清单

1. **情形表逐行**：E-1 三种帮助写法、stop --help、缺 Node、顶层帮助不变由 CLI 10+67 用例验证。E-2 三个页面产物已逐图查看；README 三个引用、仅三张文件、PNG 格式和 1512×944 尺寸、脚本说明由 7 个 Web 用例验证。正常生成及重复覆盖依照上一审计两次成功记录，本轮因 ps 受限未重现成功，不计作本轮通过。H-01 的 GitHub 观感仍由用户人工确认。
2. **失败分支**：设计 E-2 点名的缺构建分支实跑正确拒绝；E-1 缺 Node 的帮助仍成功由既有 CLI 用例验证。另实跑进程枚举拒绝，确认发生于启动服务前。拒绝授权、外部服务不可达不适用，本里程碑没有这些外部接口；连接超时及启动后清理本轮受预检阻塞未实跑，不声称覆盖。
3. **全部迁移**：不适用业务状态机，本轮仅替换静态图片。截图生命周期本轮走临时根创建→枚举拒绝→finally 删除临时根，无服务/内核/浏览器启动；正常启动→截图→关闭→残留检查引用上一审计成功记录，不冒充本轮实跑。
4. **输入域边界**：空串 --out 已实跑拒绝；三个文件逐一匹配签名与尺寸，覆盖全部交付图片；帮助的 --help/-h/help 和嵌套 stop --help 由 CLI 用例覆盖。本轮无日期、数值或业务集合输入，前导零、闰日、跨年、最大值、单元素不适用。

## 用例总数与基线

本轮项目测试不同用例总数 **84 = 10 CLI 集成 + 67 CLI 单测 + 7 Web**，最终 **84 通过、0 失败、0 跳过**。相对上一审计同范围 84，差 **0**，只有图片尺寸用例红→绿，未删改任何断言。相对上一实现轮的 22，差 **+62**：恢复帮助/bin 77 例，不再运行当轮 CI 修复的 quick 14 与 build_id 1，Web 7 不变（77−15=62）。手动参数/缺构建检查和构建不计为项目测试用例；截图脚本三次环境失败另列，不混入 84 例的通过结论。

## 收尾

未启动常驻服务、浏览器或真实账本内核；CLI 测试使用原有临时夹具。仅图片与本任务证据/状态/retro 变动，无业务源码修改。用 NO_CI=1 提交，防止 post-commit 自启本轮禁止的项目门禁。不推送、不启动下一会话。
