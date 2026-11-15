# M-11 · 实现轮 21/65 · 快检超时修复

本轮按用户末尾修复指令，不推进新里程碑。M-11 原为开放，仍保持开放；其它里程碑状态不变。上一审计 M-10 已通过，无审计退回检查需要搬入。本轮新增项目回归先红后绿。

## 定位与修复

宿主提供的日志没有失败断言：notifier-live 3、load-context 10、logtail-live-cleanup 1 已过，board-live-listen 第二例清理期间收到宿主 600 秒终止。不能据此认定第二例死锁。源码显示快检只在候选开始前检查 120 秒预算，所有 Web related 测试是一个候选，预算不能在文件之间生效；test-groups.ts 的改动关联全部 139 个文件，足以使快检变成长测试批次。

保留 Vitest 本身的关联图判定，新增 quick-related.mjs 仅发现关联文件，关闭发现实例；按直接改动测试、unit、kernel、exclusive 顺序生成逐文件命令。每个文件都走既有 ci_quick_admit / ci_partial，预算用完后的文件明确记 not_run，不发全量通过凭据。运行中的文件照常完成，单文件本身超长仍可能越过预算，本修复不改变既有软预算语义，也不杀正在清理的测试。全量门禁与测试断言不变。

## 验证与用例总数

- 红：`node --test --test-name-pattern='14 Web' scripts/test/ci-quick.test.mjs`，新增回归 1 例失败；旧代码执行整体 related 命令，没有执行逐文件 FIRST_WEB。日志 `.autome/output/r21-red.txt`。
- 绿：同一命令 1/1 通过，首文件完成后第二文件记 not_run；日志 `r21-green.txt`、最终 `r21-budget-final.txt`。
- `node --test scripts/test/ci-quick.test.mjs`：15/15 通过，0 失败、0 跳过，100.6 秒，日志 `.autome/output/r21-tests-final.txt`。首次中间版本 fixture 尚未适配发现器，失败记录在 `r21-tests.txt`，已修复替身。
- 最后补强真实发现断言后运行 `node --test --test-name-pattern='03 |04 |05 |06 |09 |12 |14 ' scripts/test/ci-quick.test.mjs`：7/7 通过，43.6 秒，日志 `.autome/output/r21-focused.txt`。09 内部仍实跑原 Vitest related 的 ci-config 12/12，并验证新发现器只生成该文件且保留预算和结果记录。
- 用例口径：项目快检脚本测试原为 14，现 15（+1 为逐 Web 文件预算回归）；09 内部既有 12 个 Web 用例不变。因此本轮唯一已执行用例 27，较同范围基线 26 增加 1。上一实现轮里程碑子集 16，本轮 27，差 +11 来自切换为快检修复子集，不是产品测试删减；重复执行不重复计数。
- 真实发现：`node apps/web/scripts/quick-related.mjs .autome/output/r21-related.sh server/main.ts` 产出 15 个文件；将参数换为 test-groups.ts，产出 139 个文件，见 `r21-all-related.sh`。此项只验证发现，不声称这些测试已执行。
- 用户指定的同一快检命令（下文）最终退出 0，5 步通过，15 秒；日志 `.autome/output/r21-quick-final.txt`。首次运行发现新 mjs 的隐式 any 与跨目录导入扩大 typecheck 范围，已用局部带类型注释的 shell 引用函数修复，原失败日志 `r21-quick.txt`。

```sh
if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi
```

当前会话未注入 AUTOME_ROUND_START / AUTOME_BASE，快检明确报告未跑关联测试；不把静态通过当作宿主带基线执行的通过。提交后的宿主快检才是最终判据。未运行 ci-local.sh 全量门禁；测试 13 只调用它的 --plan 比较缓存键，没有执行全量门禁。

## 四项自审

1. **情形表逐行**：本轮不推进 M-11 产品情形表。快检情形：关联文件发现（09）、部分通过不写完整凭据（04）、失败保留失败状态（06）、预算逐候选与逐 Web 文件检查（05、14）、基线缺失明确未跑（07），均通过。
2. **失败分支**：关联测试失败（06）、非法预算和 revision / report 路径（12）均通过；首次 typecheck 失败已修正并原命令通过。端口、ps 不是本次调度改动的依赖，未为此弱化真实测试。发现器异常由 run 记失败且不 source 未完成输出，源码核对。
3. **全部迁移**：候选→执行→partial、执行→failed、预算耗尽→not_run、缓存沿用但相关子集仍执行（13）均通过。M-11 的运行→关闭迁移本轮不复验，沿用开放状态。
4. **域边界**：0 / 前导零 / 非法预算（12）、带空格及引号路径（03）、单关联文件（09）、两个文件跨预算（14）通过。日期、最大用户文本不适用，本轮无此类输入。

## 交接

M-11 原验收的真实进程部分仍需允许 ps / 监听端口的环境复验；本轮 ps 探测仍为 EPERM，未启动面板或真实账本。所有测试命令均已退出。下一步由 Autome 重跑带起点的快检，再继续 M-11 原契约与负向对照。本轮不自行启动下一会话。
