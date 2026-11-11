# M-04 实现证据 · 实现 #25（25/30）

## 改动与结论

只把 `apps/web/test/write-gate.test.ts` 第 117、398 行的两个原始 0x00 替换为源码 `\u0000`。用 Node assert 对照 `git show 30e5d51:apps/web/test/write-gate.test.ts`：当前全文严格等于基线全文仅替换两个 NUL 的结果；分别求值两行的原字符串和转义字符串，运行时值完全相等。未改被测实现、测试名称或数量。

上一轮审计 #3 通过 M-03，无退回缺陷，故没有本轮需逐字移植的审计检查；其新增 generation 回归已经在项目测试体系内。M-04 的字节、补丁、运行时值及目标用例检查通过；Web 全量受当前沙箱监听限制阻断，依设计「风险与验证安排」第 4 条交审计环境复验，不能宣称全量通过。

## 验收命令与结果

原始输出均在 `.autome/output/m04-r25-*.txt`，未转存到任务目录。

| 命令 | 退出码 | 结果 |
|---|---|---|
| `python3 -I -c "print(open('apps/web/test/write-gate.test.ts','rb').read().count(b'\x00'))"` | 0 | `0` |
| `git diff --numstat 30e5d51 -- apps/web/test/write-gate.test.ts` | 0 | `- - apps/web/test/write-gate.test.ts`，基线含 NUL，符合裁决后的预期 |
| `git diff --text -U0 30e5d51 -- apps/web/test/write-gate.test.ts \| grep -cE '^[-+][^-+]'` | 0 | `4`，两条删除、两条添加 |
| `git diff --no-index --numstat /dev/null apps/web/test/write-gate.test.ts` | 1 | `562 0 …`，文本行数；1 表示发现差异，不是执行错误 |
| `cd apps/web && npx vitest run test/write-gate.test.ts` | 0 | 35/35 通过；负向注入还原后再次 35/35 通过 |
| `cd apps/web && npx vitest run test/write-gate.test.ts -t "a null byte is refused anywhere in a string\|looksLikeAPath catches the shapes"` | 0 | 2 通过、33 因名称过滤跳过，总数 35 |
| `npm test --prefix apps/web` | 1 | 22 文件、225 用例：198 通过、11 失败、16 因 beforeAll 失败跳过 |
| `npm test --prefix apps/desktop` | 0 | 104 用例：103 通过、1 既有跳过 |
| `npm run lint --prefix apps/web` | 0 | 通过 |
| `npm run typecheck --prefix apps/web` | 0 | 通过 |
| `npm run build --prefix apps/web` | 0 | 通过 |
| `git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` | 0 | 空，无越界改动 |

Web 失败范围：`gateway-http.test.ts` 11 个失败；`eventbus-live.test.ts` 7 个及 `server.test.ts` 9 个因初始化失败未执行，共 27 个监听相关用例受阻。日志明确包含 `listen EPERM: operation not permitted 127.0.0.1` 与 watcher `EMFILE`；没有删测试、放宽断言或加 skip。其余 19 文件 / 198 用例通过。

**数量对比：** 目标文件 35，比前轮/任务基线 35 差 0；Web 225，比上一轮审计补强后的 225 差 0，比审计补强前 224 多 1（旧重验先 settle 的 generation 回归），比冻结契约 219 多 6（M-02 审计增加 5，M-03 审计增加 1）。desktop 104，比上一轮 104 差 0；其 1 个既有跳过不变。本轮没有新增或移除用例。

## 标待审之前的自审清单（四项逐项执行）

1. **情形表逐行：** 本里程碑没有状态情形表，按 M-04 的两处替换落点检查：第 117 行 `T-1\u0000rest`、第 398 行 `a\u0000b` 均不含源码 NUL，求值与原字符串相等；对应两个具名用例各通过，全文严格等值于仅替换两个 NUL，补丁 4 行。全部满足。
2. **失败分支：** 拒绝授权、读盘失败、超时、空输入、外部服务不可达不适用：本里程碑仅修改两个测试字符串的源码表示，无新增 I/O 或失败处理分支。设计点名的 NUL 拒绝与路径判断分支由两个具名用例覆盖并通过；下列两个负向对照已实际执行。
3. **全部迁移：** 不适用：本里程碑没有状态机或多通道，未改变 W3 的状态迁移。完整 Web 测试已运行，监听受阻部分如上明确记录，不冒充通过。
4. **域边界：** 两处内部 NUL 的运行时值与原值相等；目标文件全部 35 个用例通过（包含标识符长度、空输入和路径形状等既有检查）。本里程碑未新增输入域；日期、跨年等边界不适用。

## 负向对照与还原

- 仅替换第 117 行：NUL 数为 1、补丁修改行数为 2，均不满足验收要求（应为 0、4），能拦住漏改。剩余 NUL 字节偏移 13986。实测 `--no-index --numstat` 此时输出 `562 0 …`，**与契约中“仍是 - -”的负向预期不符**；首次对该预期的断言实际失败，随后保留观测结果而不修改契约。Git 的文本统计不能代替全文件 NUL 检查；全文件字节检查仍能稳定发现漏改。
- 将第 117 行错误写为字面反斜杠 `T-1\\u0000rest`：原用例 `a null byte is refused anywhere in a string` 实测退出 1，断言 `true !== false`，证明未包含 NUL 的错误转义会被拦住。日志 `m04-r25-literal.txt`。
- 每次注入通过 finally 恢复最终文件；恢复后全文与基线仅两次替换严格相等，目标文件再跑 35/35 通过（`m04-r25-restored.txt`）。`git status --porcelain` 中测试文件仍有本轮预期的两处修改，不能要求整个工作区为空；没有遗留注入。

## 待审事项

审计环境复验 M-04 契约与 Web 全量，特别是本环境无法运行的 27 个监听相关用例。H-01..H-03 仍由用户在合并前确认，本轮未执行真实浏览器人工验收。未运行整套 ci-local，由 Autome 宿主门禁调度。
