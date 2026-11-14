# M-01 实现轮 #1

范围：A-R1–A-R3、A-V1 / V-1；无上一轮审计、无待移植的审计回归检查。本轮不推进 B/C/D。

## 实现

- `scripts/ci/lib.sh` 统一执行、同步输出与捕获真实退出码，失败后继续其它步骤；显式 `--cases cargo|vitest` 声明解析器。
- `scripts/ci/summary.mjs` 使用 Node 标准库处理 Cargo 目标 / failures 清单、Vitest FAIL 行和 Unicode 字节预算。作为 lib.sh 的小型实现辅助文件，避免 POSIX shell 依赖平台不同的字符截断行为；没有新增依赖。
- 本地 Rust 命令及 CI 三个 Rust 测试步骤加 `--no-fail-fast`；本地 Scripts 步骤及 CI scripts job 纳入脚本回归。
- 汇总保留原步骤行，`==> 汇总` 隔离指纹；用例行 ≤160 字符且 ≤200 UTF-8 字节，标题和用例共享 2400 字节预算 / 最多20条用例，准确报告未列出数量，尾部在3000字节以内。
- README、技术设计 §16 同步行为；新增指纹与 CI 配置回归。Vitest 静态夹具取自本轮 Vitest 5.1.0 两个真实失败文件的 FAIL 行，脚本 CI 无需安装 Web 依赖即可验证解析。

## 验收与基线

验收命令（顺序执行、各自保存原始输出）：

`node --test scripts/test/ci-summary.test.mjs && cargo test -p autome-domain --test gate_failure_window && (cd apps/web && npx vitest run test/ci-config.test.ts)`

退出码 0。脚本 7/7，Rust 6/6，Web 9/9；总数 **22**，失败0，跳过0。上一轮基线采用设计已核定的 0+5+8=13（本任务首次实现，没有上一实现轮实跑）；差 **+9** 来自新脚本7例、指纹1例、CI配置1例，没有删除或跳过既有测试。内部临时 crate 的2个预期失败是脚本第1例的夹具，不另计项目测试总数。

原始输出：`.autome/output/m01/acceptance-{scripts,rust,web}.log`。脚本约1.96s、Rust测试体0.00s、Web约0.24s；这只是验收子集，不冒充完整门禁性能数据。A的收益是一次发现多个失败目标，不宣称单次门禁提速。

`git diff --check`、`sh -n scripts/ci-local.sh scripts/ci/lib.sh` 退出0。未运行全量门禁、fmt/clippy 或 Web lint/typecheck 快检，按本轮用户指令留给宿主。

## 标待审前四项自审清单

1. **情形表逐行**：A没有独立情形表，以 A方案四点与 M-01 七条验收逐行执行。
   - ① 真实临时crate两个目标各失败一次，汇总同时有 first/second 目标与用例，退出1：通过。
   - ② 两个真实Vitest失败输出，保留文件与用例；另验带ANSI颜色的整文件失败：通过。
   - ③ 40个ASCII失败仅列20条，另有20个；模拟最后60行/6000字节仍保留标题、最后列出的用例和结论：通过。
   - ④ 原ok/FAILED步骤行逐字匹配，新行不冒充失败步骤；指纹新增用例同时验证最后一步成功/失败：通过。
   - ⑤ 失败后继续后续步骤；全部成功退出0、混合失败退出1：通过。
   - ⑥ 无解析器的Clippy类失败不输出用例块：通过。
   - ⑦ 18个名字各为4个ASCII字符+156个汉字；逐行字符边界、200字节、总预算、实际未列出数量及tail保留均验证：通过。
2. **失败分支**：Rust多目标失败、Vitest用例与整文件失败、无解析器失败、解析器收到空失败输出均已执行。授权拒绝、外部服务不可达、读盘失败与超时策略不适用：A未定义这些分支，也未引入这些服务或状态；不把它们虚报成通过。真实Vitest夹具首次生成的config相对路径错误已改为绝对路径，最终真实运行确认2文件/2用例失败，见 `vitest-real.log`。
3. **全部迁移**：本轮无持久状态机/多通道；执行器临时状态已逐项覆盖：初始→成功、初始→失败、失败后→后续成功、全成功→退出0、有失败→退出1、零步骤→退出0；成功步骤即便输出failure样本文本也不计失败用例。没有实现沿用迁移（属于M-02）。
4. **输入域边界**：零步骤、单步骤、空输出、40条超过20条上限、18个长中文名触发字节上限、ASCII名称、ANSI颜色、整文件失败与多个Rust目标均已执行。日期/闰日/数值最大值不适用：本轮输入为测试日志，不解析日期或外部数值参数。

## 负向对照与恢复

- 复制引擎/脚本测试到 `.autome/output/m01/mutations/`：去掉测试调用的 `--no-fail-fast` 后第1例红（只运行一个目标）；取消200字节限制仅按字符截断后第7例红；用例行改为FAILED前缀后格式检查红。三次均退出1；见 `mutations.log` 及其子目录日志。
- 原新增Rust指纹回归分别临时去掉汇总分隔头、把用例行改为FAILED前缀，单跑 `cargo test -p autome-domain --test gate_failure_window gate_failure_window_case_summary_preserves_fingerprint` 均退出101，断言抓到指纹变化；见 `fingerprint-mutations.log`、`no-summary-header.log`、`failed-case-prefix.log`。源文件在finally中恢复。
- 恢复全部改动后完整验收子集22例全绿。负向对照属于同一批用例的缺陷注入运行，不重复计入用例总数。

结论：M-01具备待审证据；全量宿主门禁和宿主快检未实跑，本轮无环境阻塞。

写入四项清单后、修改状态之前再次执行同一验收命令：退出0，仍为22例；日志 `self-review-{scripts,rust,web}.log`。
