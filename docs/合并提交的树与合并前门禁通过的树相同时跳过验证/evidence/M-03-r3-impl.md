# M-03 实现轮 #3

范围：CLI MainCheck 复用文案、README、技术设计和全量门禁。上一轮 M-02 审计通过，没有退回缺陷，因此无须搬入审计缺陷检查。未改调度器、状态机或既有测试断言；全量发现 Web 通知夹具依赖 MainCheck 必跑，已补显式关闭开关。

## 实现与修复

- 在通用 `gate.reused` 分支之前增加 MainCheck 守卫，输出任务卡指定句子。
- 新增 `main_check_reuse_is_printed_by_autome_run`，同时断言 MainCheck 新句和 MergeGate 原句。
- README 配置示例与 Host gate 说明补充默认开启、关闭方法、提交/命令匹配及不建验证检出；技术设计同步。
- 首次 render 检查 15 通过、1 失败，原因是新增测试把既有 SHA 缩写长度误写成 8（实际 10）。修正期望为原有 10 字符，未修改缩写实现；再次检查 16 通过。日志：`.autome/output/m03-render.txt`、`m03-render-green.txt`。

## 验收命令与基线

- `cargo test -p autome-cli render::`：16 通过，相对 15 基线 +1（新增双文案断言的一个测试函数）。最终日志 `.autome/output/m03-final-render.txt`。
- 分文件计数阈值检查：README 2 ≥ 2，technical-design 1 ≥ 1，通过。
- `cargo test --workspace`：退出 0，注册总数 1224 = 1223 通过 + 1 ignored，0 失败；上一轮审计基线 1223，差 +1，全部来自新增 render 测试。ignored 仍为 `t14_gate_logs`，需要外部 `AUTOME_T14_GATE_DIR`；没有新增忽略。日志 `.autome/output/m03-workspace.txt`。
- `sh scripts/ci-local.sh`：退出 1，用时 506 秒。Format、Clippy、Rust 全量、Contract、Web lint/typecheck/build 通过；Desktop 与 Web test 失败。日志 `.autome/output/m03-ci.txt`。
- ci Rust 仍为总数 1224；Desktop 105 = 104 通过 + 1 失败；Web 364 = 331 通过 + 14 失败 + 19 skipped（43 文件，6 失败、37 通过）。三层共注册 1693 个，不能将失败初始化导致的 skipped 当作覆盖。上一轮只记录 Rust 基线；Desktop/Web 无上一轮实跑计数可比，未修改注册用例，不能声称其差为 0 已实证。
- Desktop DOM harness：Electron 退出 `SIGABRT`、status=null，断言位置 `apps/desktop/test/dom.test.js:83`。当前权限不能提权，未设置跳过变量。
- Web live/server/gateway/shell 测试：`listen EPERM: operation not permitted 127.0.0.1`、启动等待超时及 `EMFILE: too many open files, watch`；属于当前运行环境阻塞。
- Web notifier-real 两项失败另有明确原因：账本夹具预期 MainCheck 失败生成 `project_alert`，新默认复用使它少 1 条通知（14 → 13）。在 `w4-ledger.sh` 的临时项目配置中增加 `reuse_for_main_check = false`；不删报警断言、不减通知数。首次红证据见上述 ci 日志，修复后的单独复验见下。
- `cargo fmt --all -- --check` 与 `git diff --check` 通过；未运行可选 package。


## 标待审前自审清单

1. **情形表逐行**：方案 §5 的 MainCheck → 指定新句、MergeGate → 原句，由新增测试逐项严格相等断言；task.get.gate / status / 桌面与 Web 读者按设计不改。README 示例和说明分别出现开关，技术设计出现开关，分文件阈值检查通过（README ≥ 2、技术设计 ≥ 1），避免原多文件 grep 的退出码漏洞。
2. **失败分支**：本里程碑无授权、读盘、超时或外部服务调用，运行时失败分支不适用；设计点名的文案负向检查另记下方。M-01/M-02 的取树失败、缺记录、命令变化及准备失败等由全量项目测试复验。
3. **全部迁移**：本轮只改纯事件渲染与文档，不新增状态迁移，不适用；MainCheck 和 MergeGate 两个事件通道已逐一断言。既有状态机与 e2e 纳入全量测试。
4. **输入域边界**：本轮无日期、数值或新增输入域，不适用；事件区分边界是 `node == main_check`，正向与 `merge_gate` 负向均测试，新增事件字段与旧文案兼容。未改变既有 SHA 缩写边界。

## 负向对照

三项变异均使新增用例失败：专用守卫禁用（等价于不加分支）、专用分支移到通用分支之后、守卫对所有 node 生效。对应日志 `.autome/output/m03-negative-missing.txt`、`m03-negative-order.txt`、`m03-negative-all_nodes.txt`；临时脚本均通过 ensure 还原源码。最终 `cargo test -p autome-cli render::` 16/16 通过（`.autome/output/m03-final-render.txt`）。这些重复和变异执行不增加注册用例数。文档阈值按文件独立断言，README 为 2、技术设计为 1；并同步修正设计 Backlog D1-B01 所指验收命令。

## 本轮结论

`npm run test --prefix apps/web -- test/notifier-real.test.ts` 修复后退出 0，2/2 通过，55.87 秒；日志 `.autome/output/m03-notifier-fixed.txt`。与同一用例在全量中的 2 失败相比转绿，没有增减测试或改变断言。全量门禁未在受限环境中取得通过，M-03 保持开放，reopen 保持 0，implementation-round 为 3/15。下一轮在允许 Electron 启动、本地监听的环境重新执行全量门禁；不宣称整体全绿，不标待审或已完成。

状态字段唯一性、轮号、M-03 开放状态和三条证据指针已检查通过。未修改 `.autome/` 配置/规则、未切分支、未推送、未启动下一会话；原始日志与临时检查仅放 `.autome/output/`。
