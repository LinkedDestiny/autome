# M-02 · 实现 #3

## 范围与实现

本轮仅推进 M-02。上一轮审计 M-01 通过，无缺陷退回、无须逐字迁入的审计复现检查。

新增宿主 `merge_after` 模块：Cleanup 主检出准备后启动真实 gate wrapper，传入任务 ID 和完整合并提交；轮询时保持 worktree，结论后才清理。用持久事件防止重复结论，已有作业缺开始事件时补写；Vanished 清文件并在下一 tick 重试。失败警报、项目事件、任务结果在同一 SQLite 事务提交。CLI 支持开始、通过、未通过、跳过四种文案。准备完成标记避免钩子等待时重跑准备。

M-03 的跨任务互斥、停止恢复以及相关工作区/Bug/准备协作验收仍留给下一里程碑；本轮不宣称整项功能完成。

## 命令与结果

原始输出保存在 `.autome/output/m02-*.txt`，不转存至任务目录。

- `cargo test -p autome-cli > .autome/output/m02-cli.txt 2>&1`：26 + 13 = 39 个通过，较 25 + 13 基线增加 1（R1），0 失败、0 忽略。
- `cargo test -p automed --test end_to_end merge_after > .autome/output/m02-e2e-filter.txt 2>&1`：3 个通过，较上一轮 0 增加 3（E1–E3）；103 个被过滤，不是跳过。
- `cargo fmt --all -- --check > .autome/output/m02-fmt-check.txt 2>&1`：退出 0。
- `cargo clippy --all-targets -- -D warnings > .autome/output/m02-clippy.txt 2>&1`：退出 0。
- `git diff --check`：退出 0。
- `cargo test -p automed --lib merge_after > .autome/output/m02-lib-filter.txt 2>&1`：8 个通过，较上一轮 4 增加 4（L1/L2/L4/S1）；647 个被过滤。
- `cargo test -p automed --test end_to_end > .autome/output/m02-e2e.txt 2>&1`：106 个通过，较基线 103 增加 3（E1–E3），0 失败、0 忽略。
- `cargo test -p automed --lib > .autome/output/m02-lib.txt 2>&1`：655 个通过，较上一轮 651 增加 4（L1/L2/L4/S1），0 失败、0 忽略。
- 相关完整目标总数：106 + 655 + 26 + 13 = **800**，上一轮基线 103 + 651 + 25 + 13 = **792**，净增 **8**（e2e 3、lib 4、CLI 1），没有删改现有用例。过滤运行是同一批用例的重复验证，不加入总数。
- `git diff --exit-code HEAD -- .autome/`：退出 0，未修改受跟踪的 Autome 配置/规则；仅按协议写入忽略的输出文件。

以上四项自审已随最终测试执行通过，结论：M-02 待审。全工作区测试与 ci-local 按契约留给 M-04，本轮未宣称通过。

## 标 `待审` 之前的自审清单

1. **情形表逐行**：M-02 对应执行与打印情形逐条检查：配置且 MainCheck 通过 → E1，主检出产物、完整提交环境变量、worktree 尚在、事件顺序、日志、主检出干净；非零退出 → E2，一条警报、Done、worktree/分支删除；未配置/空白命令 → E3，无事件、无作业/准备标记；开始/成功/失败/跳过打印 → R1。方案跳过表的 pure plan 已由 M-01 覆盖，集成跳过与协作属于 M-03。
2. **失败分支**：退出 6 → E2；超时 → L1，改 `.started` 为 1，断言 124、超时日志、一条警报、wrapper 与 sleep 子进程均终止；SQLite 结果插入失败 → S1，触发器拒绝 `merge.after` 后警报和两个事件计数不变；Vanished → L2 内部子场景，未知结果不报警、下一 tick 重启。拒绝授权、外部服务不可达不适用：钩子没有授权或外部服务协议，外部命令失败统一由退出码表达。文件 I/O 错误沿现有 Result 向上传播，无本里程碑专门的恢复契约。
3. **全部迁移**：未启动 → Pending（E1/L2）；Running → Pending（L2）；Done(0) → Cleanup 完成（E1）；Done(6/124) → 警报并 Done（E2/L1）；结论已写 → 直接 Done、不重跑（L2）；已有作业无开始事件 → 补写并收集（L4，追加记录只有一行）；Vanished → 清文件/Pending → 重启 → Done（L2）；运行时删除配置 → 不等待地 Done、后台作业自行结束且无结果事件（L2）。暂停/停止/取消协作属于 M-03，本轮不作为已验收项。
4. **域边界**：缺省配置与全空白命令（E3）；有门禁通过（E1）与无门禁（L1/L2/L4）；完整 40 位提交传入环境、作业名取前 12 位（E1/L4）；退出码 0、6、超时保留码 124（E1/E2/L1）；一条作业、一条结论、一条失败警报（E1/E2/L2/S1）。正整数超时解析边界属于已通过的 M-01，本轮用 1 分钟验证运行时边界。日期、闰日、前导零不适用：无日期或文本数字输入域。

## 修复与验证过程

首次编译发现新增测试位于 `needs_git!` 宏声明之前、e2e 的 Path 未导入；分别移动到宏后和使用完整类型路径。均为本轮测试编译问题，不是上一轮审计回归。后续验收以最终代码为准。

lib 过滤首跑还发现夹具 config.toml 可能不存在，3 项在读取配置处失败、尚未启动钩子；修正为与现有 scheduler 测试一致的空配置默认值后，8 项全绿。红日志 `.autome/output/m02-lib-fixture-red.txt`，绿日志 `.autome/output/m02-lib-filter.txt`。
