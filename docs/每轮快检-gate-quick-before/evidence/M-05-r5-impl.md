# M-05 · 实现 #5

结论：本轮验收通过，提交审计。implementation-round 为 5/25；只推进 M-05。上一轮 M-04 审计通过，没有退回缺陷，因此没有需要逐字迁入测试的审计复现检查。

## 改动

- CLI 按实际结果 kind 渲染快检与修复门禁（复现 + 快检），抽出 gate_status_line；快检超时不再拼出“快检门禁超时”。非 quick 的既有文字保持不变，包括缺失节点在事件与 status 中各自的原回退值。
- 桌面抽出纯函数 gateTagLabel，hostTags 调用；原颜色、悬停命令与日志路径保留。
- README 与技术设计补齐两级门禁、推荐范围、超时、并发排队和 AUTOME_GATE_KIND；修正无 quick 时才跳过 Gate 的说明。本仓库配置未修改。
- 新增 CLI 2 个、桌面 3 个测试；没有修改任何既有测试断言。task.get、路由、Web、JSON 输出路径不改。

## 命令与结果

1. `cargo test -p autome-cli -- render::tests::gate_`：退出码 0，11 个通过，0 失败，0 跳过；另一个集成测试二进制因筛选执行 0 个（16 filtered out），没有把它计入通过数。原始输出 `.autome/output/m05-cli.txt`。
2. `node --test apps/desktop/test/labels-gate-kind.test.js`：退出码 0，3 个通过，0 失败，0 跳过。原始输出 `.autome/output/m05-desktop.txt`。
3. `sh -c 'grep -q AUTOME_GATE_KIND README.md && grep -q AUTOME_GATE_KIND docs/development/technical-design.md && grep -q quick_timeout_minutes README.md && ! grep -q "Bug fix rounds run only the repro command;" README.md && ! grep -q "Bug 修复轮只跑复现命令，FixGate" docs/development/technical-design.md'`：退出码 0。
4. `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`：最终复验退出码均为 0，输出为 `.autome/output/m05-final-fmt.txt` 与 `m05-final-clippy.txt`。

本轮测试用例总数 **14**。在上一轮提交上先实跑同一 CLI 子集取得基线 **9**（`.autome/output/m05-baseline.txt`），桌面新文件原先不存在；同范围净增 **5**（CLI +2、桌面 +3）。上一轮 M-04 报告的测试总数为 **56**（lib 25 + prompt_rendering 31），本轮较其 **-42**：本轮移出该 56 个与 M-05 无关的测试，纳入本里程碑 14 个，非用例丢失或跳过。文档 grep、静态检查及协议评估不计入测试框架用例数。

## 标待审之前的自审清单

1. **情形表逐行**（设计 §10）：
   - autome run：需求 / FixGate × started / result；通过、普通失败、退出码 124 但无内核终止标记、内核超时，均由新增 CLI 用例逐项断言完整字符串，通过。
   - autome status：需求 / FixGate × 通过 / 未通过 / 超时，新增用例断言完整行，通过。
   - task.get.gate：检查接口仍透传原结果；本轮未改 dispatch，kind 来源保持前序里程碑实现。
   - 桌面 hostTags：调用新函数；3 个纯 Node 用例覆盖需求、FixGate、full/repro/未知/无 kind，对照通过；命令与日志 tooltip 保持原实现。
   - routing.js、Web：按表不改；git diff 确认无改动。
   - JSON 输出：本分支没有本轮新增的 JSON 路径改动；T-46 合并后检查仍按设计留待宿主 rebase 后进行，不声称本轮已完成该检查。
   - gate.queued / reused / skipped：未修改分支，既有 gate_ 测试通过。
   - §11 文档：示例、两级取舍、each_round 忽略、quick 不替代完整门禁、项目决定是否包含快检内容、环境变量三种值、共享队列均逐项核对；grep 验收通过。
2. **失败分支**：渲染范围中的未通过、普通 exit 124、内核超时均实际跑过。拒绝授权、读盘失败、外部服务不可达不适用：本轮纯渲染函数不访问文件/网络、不执行授权或作业；这些故障已有宿主路径，本轮不修改。缺失字段以空结果测试覆盖。
3. **全部迁移**：不适用新增状态机（本轮没有状态迁移）；读者通道按 run/status/desktop 分别检查，需求与 FixGate 的 quick/full/repro/未知/缺失 kind 均覆盖。started 与成功/失败/超时结果均检查；没有用快检文字覆盖非 quick 结果。
4. **输入域边界**：kind 缺失、未知、full、repro、quick；sha 空串、单字符、长 SHA（桌面截 7、CLI 截 10）；缺失命令/日志/ok 与空对象，均由新用例或既有 short 测试契约核对。日期、前导零数字、闰年不适用：本轮无该类解析。CLI 空对象新增断言通过。

## 负向对照

- 暂时禁用 gate_label 的 quick 判断，运行 `cargo test -p autome-cli -- render::tests::gate_quick_run_lines_name_the_quick_run`，退出码 101（预期红）；还原后上述 11 个全绿。输出 `.autome/output/m05-negative-cli.txt`。
- 暂时禁用 gateTagLabel 的 quick 判断，运行桌面验收命令，退出码 1（需求与 FixGate 标签失败）；还原后 3 个全绿。输出 `.autome/output/m05-negative-desktop.txt`。
- 暂时恢复 README 的旧句，再原样运行文档验收命令，退出码 1；还原后退出码 0。
- 三处变异均已还原，未留下故意损坏的实现。

## 热缓存耗时

任务卡命令：`cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web`。

首次预热因尚未安装 Web 依赖，在 lint 处退出 127（eslint not found）；`npm ci --prefix apps/web` 退出 0，锁文件未改。随后完整预热退出 0，再以 `/usr/bin/time -p` 测各段及整条，均退出 0：

| 检查 | real 秒 |
|---|---:|
| cargo fmt | 0.93 |
| cargo clippy | 0.27 |
| Web lint | 1.90 |
| Web typecheck | 4.86 |
| 四段整条独立实测 | 6.00 |

各段与整条为独立采样，缓存与调度不同，不要求和相等。输出 `.autome/output/m05-time-{fmt,clippy,lint,types,total}.txt`；预热为 `m05-warm2.txt`。这是当前机器当前缓存的一次测量，不是稳定性能保证。最终 Rust 改动另跑 fmt/clippy 均通过。

未跑全量门禁 `sh scripts/ci-local.sh`，未启动 Electron；H-01/H-02 仍由用户在合并前后按清单确认，本轮纯标签测试不冒充真实桌面验收。
