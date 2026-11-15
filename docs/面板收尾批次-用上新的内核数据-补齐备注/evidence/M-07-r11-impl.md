# M-07 · 实现 #11

结论：**未通过，保持开放**。E-1 已实现并取得通过证据；E-2 的脚本、README 与图片守卫已落地，但本沙箱不允许 `ps`，两次生成均在启动资源之前失败，三张图片仍是旧图，尺寸守卫明确失败。没有把旧图缩放或伪造为新截图，没有标待审或已完成。implementation-round 为 11/65，reopen 仍为 0（不是审计退回）。

## 范围与实现

- `web.rs::run` 在 Node/面板查找之前处理任意位置的 `--help`、`-h` 与首参数 `help`，中文用法包含设计要求的全部公开选项；不列内部 serve/instance。`main.rs` 的 USAGE 与原逐字单测未改。
- CLI 新增三例：三种别名；没有 Node 和面板时仍可帮助；子命令之后帮助与顶层帮助相同（含未知子命令后帮助）。保留原七例。
- `scripts/screenshots.ts`：先检查生产构建和两个当前检出的二进制；使用临时 HOME/AUTOME_HOME、w7-flow.sh、假 CLI，新增四条业务任务，真实内核推进一条至设计审批，其余草稿；production 服务 port 0；无头 Chromium 1512×944；依次取总览、项目、任务；finally 关闭浏览器与服务、清理并检查残留、删除临时根。不能检查进程则拒绝启动。默认输出 docs/images，可传 --out；未接门禁。
- README 仅保留总览、项目、任务三个引用及新脚本入口；删除 routing/skills/environment 三张不在要求内的旧图。dashboard/project/task **未生成替换**，仍分别是 1200×451、1200×742、1200×749。
- 新 readme-images 三例加入 unit；图片文件名单、引用、PNG 格式及尺寸都被检查。未修改 `.autome/` 配置或协议；仅按用户指定在 `.autome/output/` 写命令日志。
- 上轮审计 M-06 通过、无 M-07 缺陷检查，故无需要逐字搬入的审计回归用例。

## 命令、结果与用例数

原始输出均在 `.autome/output/`，这里仅记结论。

| 命令 | 结果 | 日志 |
|---|---|---|
| `cargo test -p autome-cli --test cli web_`（改前） | 7/7 通过 | m07-baseline-cli.txt |
| `cargo test -p autome-cli --bin autome`（改前） | 67/67 通过 | m07-baseline-bin.txt |
| `cargo test -p autome-cli --test cli web_`（还原负向变异后） | 10/10 通过 | m07-cli-final.txt |
| `cargo test -p autome-cli --bin autome`（改后） | 67/67 通过 | m07-bin-final.txt |
| `cargo build -p automed -p autome-cli` | 退出 0 | m07-build-core.txt |
| `npm run build --prefix apps/web` | 退出 0 | m07-build-web.txt |
| `node apps/web/scripts/screenshots.ts --out .autome/output/shots-1` | 退出 1：spawnSync ps EPERM | m07-shots-1.txt |
| `node apps/web/scripts/screenshots.ts --out .autome/output/shots-2` | 退出 1：相同错误 | m07-shots-2.txt |
| `(cd apps/web && npx vitest run test/readme-images.test.ts test/test-groups.test.ts)` | 7 例，6 通过、1 失败；旧 dashboard 尺寸 1200×451 不符 | m07-images.txt |
| `(cd apps/web && npx vitest run test/core-sidecar.test.ts -t '测试源码不按条件跳过真实内核用例\|浏览器测试与配置不按条件跳过用例')` | 两条守卫通过；其余 17 条因显式名称过滤未执行，不计为已验收 | m07-skip-guards.txt |
| `(cd apps/web && npx eslint scripts/screenshots.ts test/readme-images.test.ts)` | 修正 finally 内直接 throw 后退出 0 | m07-eslint.txt |
| TypeScript compiler API 检查新增两文件的语义诊断 | 0 条（仅报告本轮新增文件，不宣称全项目 typecheck 通过） | m07-types.txt |
| `cargo fmt --all`、`git diff --check` | 通过 | 无原始长输出 |

**验收测试总数 84 = CLI 10 + bin 67 + Web 7，相对同范围改前基线 78 = 7 + 67 + 4，增加 6：CLI 帮助 3、README 图片 3。结果 83 通过、1 失败、0 条条件跳过。** 加上另跑的两条源码守卫，本轮实际执行的不同测试为 86 个（85 通过、1 失败），同范围基线 80，仍 +6；过滤的 17 条不算已执行。负向变异与重复执行不重复计数。上轮 M-06 报告的 52 个（node 26、vitest 12、Playwright 14）属于另一验收范围，与本轮主验收 84 差 +32，来自里程碑切换，不能声称新增了 32 条测试。

此 crate 只有 bin target，没有 lib；按验收契约跑完整 `--bin autome`。没有改 apps/web/app 或 server，故不运行 related。按本轮用户指令，未执行 ci-local、ci-quick、全项目 clippy/lint/typecheck 门禁；宿主负责快检与合并门禁。

## 负向对照

1. 暂时禁止 Rust `--help/-h` 分支，执行 `cargo test -p autome-cli --test cli web_help_without_node_or_panel`：**红**，预期 0 实际 1（m07-help-negative.txt），证明没有 Node 的机器无法依赖 Node 侧帮助。随即还原原文件并重跑 web_ 全部 10 例：**绿**。变异不留在提交内。
2. README 仍使用旧图时尺寸检查：**红**，明确报告 dashboard 1200×451 对 1512×944（m07-images.txt）。这一失败尚未修复；必须由真实截图生成补齐，不能改弱断言。

## 标待审之前的自审清单（四项逐项执行）

1. **情形表逐行**：E-1 三种帮助别名、子命令后 help、无 Node/无面板、全部公开命令与选项、内部参数不暴露、顶层 help 不变均由 10+67 用例验证。E-2 生产构建存在已验证；四条业务任务、三页截图、清理后无残留是脚本实现但尚未实跑到达；README 三引用与文件集合通过，尺寸失败；两次重复生成均被 ps 拒绝。因此本项不通过。
2. **失败分支**：分别临时移走并恢复 `apps/web/build/server/index.js`、`target/debug/autome`、`target/debug/automed`，运行默认截图命令均退出 1 并提示先构建（m07-missing-index.js.txt、m07-missing-autome.txt、m07-missing-automed.txt），三个文件已原位恢复；无 Node/面板帮助通过，普通 web 的无 Node/无面板拒绝由原测试覆盖；进程检查拒绝实测两次。内核连接超时、浏览器启动失败和截图写盘失败后的清理尚无运行证据，受前置进程检查阻塞。不涉及外部服务、授权弹窗。
3. **全部迁移**：帮助是无状态分支，不适用业务状态机。脚本资源生命周期：参数/构建失败→未创建临时根已跑；创建临时根→进程检查失败→finally 删临时根已跑（未起服务、内核、浏览器）。夹具→服务→浏览器→三张截图→全部关闭/残留为零尚未跑到，不能据静态 finally 声称通过。
4. **域边界**：`--out ''`、未知参数均退出 1 给出用法（m07-empty-out.txt、m07-invalid-out.txt）；无参数默认目录在三次缺构建检查中使用；指定 out 的两次运行因 ps 受阻。帮助在第一位置与后置均测；无参数 web 转发由原测试覆盖。日期、前导零、闰日、跨年、最大数值不适用，本里程碑没有这些输入域；端口固定为 0，尺寸固定 1512×944。

## 阻塞与交接

- 当前环境禁止进程枚举，不能满足 E-2 真截图交付；没有尝试绕过沙箱。两次脚本都在进程预检处失败，未启动任何服务或浏览器，临时根已在 finally 删除。不能把这一点冒称为完整成功路径的无残留验收。
- 下一轮在允许 ps 与监听端口的环境跑默认截图生成（替换 docs/images 的三张图），再跑契约中 shots-1、shots-2 与 readme-images/test-groups；检查三图，成功后才可标待审。H-01 仍由人确认。
- 提交尝试见本文件末尾；若沙箱拒绝则提交由 Autome 兜底。

提交结果：`git add` + `git commit` 已成功，当前分支 `autome/面板收尾批次-用上新的内核数据-补齐备注`；未切分支、未推送、未启动下一会话。
