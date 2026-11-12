# M-03 实现 #4：宿主门禁失败复查

结论：原 End to end 失败未复现；完整门禁因当前沙箱 Web 监听限制失败，未取得全绿证据。保持 M-03 待审，不推进 M-04，不宣称根因已修复。implementation-round 为 4/25。

## 原失败与修复判断

- Autome 在 ad5e78d0cf 上的原始日志（只读）：主仓库 .autome/output/gates/T-28/gate-ad5e78d0cf*.log。失败是 bug_loop_fix_rounds_run_out_and_extend_adds_more，第 4427 行期望 Some("await_merge")，实际 None；该阶段 79/80 通过。
- 原日志未打印任务的失败原因、会话生命周期和事件序列，None 不能区分熔断、会话异常、协议失败等状态。本轮没有复现相同失败，不能凭该断言归因于熔断。
- 检查该用例和生产链路：前两次修复不改产品，可复用相同 SHA 的失败作业；FixBudget 后 extend 启动修复会话，BUG_FIX 改产品并提交，新 SHA 的门禁应通过。record_gate_result 仅对失败结果计数，成功结果不走熔断分支。本轮真实运行符合这一链路，但这不是原偶发失败的机制性解释。
- 未修改生产代码、既有测试或门禁脚本，也未通过提高超时、关闭步骤、改断言掩盖问题。上一轮审计为通过，没有待迁入的审计判别检查；本次 Autome 抓到问题的检查本来就在项目测试体系，已原样重跑，没有获得本轮红→绿证据。
- 后续若 Autome 再现，应采集该断言前完整 task.state、session 生命周期、gate.result 和 session.started 顺序，再定位；原偶发失败的因果证据仍未闭合。

## 命令与结果

1. `sh scripts/ci-local.sh`：退出 1，564 秒。日志 `.autome/output/r4-ci-before.txt`。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 均返回成功；Web test 失败。未运行可选 package。
2. Rust workspace all-targets：1125 个通过、1 个忽略，总数 1126。与 r3 后提交的基线差 0；与设计初始 1084 个相比增加 41 个普通用例（M-01 16、M-02 13、M-03 12）及 1 个忽略的 T-14 测试。automed 库仍 618 个，比上一轮 618 差 0。
3. End to end：workspace 阶段 80/80，单独阶段 80/80，均无忽略；总数相对上一轮 80 差 0。Adapter flags 单独 1 个通过（过滤 6），是已有测试重复执行，不算新增。
4. Desktop：总数 104，103 通过、1 跳过，较 Autome 原日志总数差 0。真实 Electron DOM harness 因二进制缺失且下载失败跳过；原日志也有相同跳过，不能视为真实 DOM 验收。
5. Web：总数 225（较上一轮 225 差 0），198 通过、11 失败、16 跳过。直接错误为 `listen EPERM: operation not permitted 127.0.0.1`，server 套件还有 `EMFILE: too many open files, watch`，eventbus 启动等待超时。当前权限策略不允许提权；未改 apps/web，也未绕过监听限制。
6. 原用例原样单跑：`cargo test -p automed --test end_to_end bug_loop_fix_rounds_run_out_and_extend_adds_more -- --exact --nocapture`，1/1 通过，过滤 79；`.autome/output/r4-regression-before.txt`。同命令再串行执行 10 次，全通过，日志 `.autome/output/r4-repeat-1.txt` 至 `r4-repeat-10.txt`。连同完整门禁中的两次，该旧用例本轮共 13 次通过；重复次数不改变用例总数。

## 标待审之前的自审清单

本轮维持既有待审状态，以下检查仍逐项执行，结果来自上述 workspace 全量；未新增测试，未借此关闭里程碑。

1. **情形表逐行**：M-03 原 12 个测试全过。计数表无历史、通过、旧事件、已熔断、不同指纹均从 1 起，同指纹累加；gate.reused 清零；其它任务与节点隔离。节点表 gate/merge_gate/fix_gate 追加字段，repro_check/main_check/通过不追加字段，均由已有 same_failure 用例覆盖并通过。
2. **失败分支**：已有日志不可读、空日志、超时、非法配置检查随全量通过；授权拒绝和外部服务不可达不适用（本轮没有新增此类接口）。Web 本地服务启动不可达实际失败，环境阻塞如上，不将其算作通过。
3. **全部迁移**：已有已熔断重放、未熔断重放、新 session.started / gate.started 尝试边界、gate.reused 重置、熔断后的 rerun/extend、预算边界均随 domain 374 和 automed 618 全量通过。旧 Bug FixBudget→extend→修复→await_merge 另外原样复跑 13 次通过。没有取得原失败时的中间状态，不能声称已解释偶发迁移失败。
4. **输入域边界**：已有配置缺省 3、最小 1、最大 10、0/11/负数/非整数拒绝；日志空、单行、400 行与 256 KiB 窗口等用例通过。旧预算用例中的 extra_rounds=0、u32::MAX 拒绝及 +1 继续均通过。闰日、跨年、日期与本轮接口无关，不适用。

## 交付与阻塞

仅更新当前 worktree 的任务记录；M-03 保持待审，未修改其它里程碑状态。未推送、未启动下一会话。全量门禁以 Autome 对本轮提交的复跑为准；本地环境限制和未复现的原失败均不是协议失败。
