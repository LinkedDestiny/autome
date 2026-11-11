# M-03 · 实现 #4

范围：仅 M-03，统一 run_gate 的准备前置阶段和 MainCheck 合并提交检出。上一轮审计 #2 通过 M-02，没有退回缺陷或须逐字迁入的判别检查。

## 实现与检查中的修正

- 门禁作业不存在且未复用时调用现有 setup::ensure；Pending 保持节点，Ready 启动门禁，任务检出 Failed 使用 SetupFailed，不退回实现轮。
- MainCheck 使用独立 verify 指纹；新建检出时失效旧记录，完成或准备失败时清理检出与记录。准备失败写 setup_failed 项目报警及 gate.skipped，然后进入 Cleanup。
- 首次编译暴露 ensure 的 boxed error 不能直接转 SchedulerError，按现有会话入口映射修正。
- 首轮新增 e2e：13 个中 11 通过、2 失败。失败来自测试误以为审计/复盘后提交不变；实际三个 gate.started 包含 MergeGate，Bug 同理不复用。修正为断言 MergeGate 重新门禁但不重新准备，另用调度器矩阵专门检查同 SHA 的复用。没有放宽准备次数或先后顺序断言。

- 全量 e2e 首次 93/93 通过；同时执行的过滤测试却出现 E6 运行 3 次准备（12/13 通过）。独立单跑 E6 随后通过，故未把重跑通过当作结案依据。
- 检查共用 gate::poll_as 发现实际跨语句窗口：第一次读 exit 不存在 → wrapper 原子发布 exit 并退出 → pid_alive=false → Vanished → setup 清理并重新启动。这也影响原门禁，修复位于共用轮询函数：返回 Vanished 前重新读取退出标记。
- 项目测试新增 `gate::tests::setup_exit_published_between_marker_read_and_liveness_check_is_collected`：用 FIFO 的读写握手确定性地在首次读标记之后发布 exit、再返回已死 pid，不靠概率或 sleep。原实现红（0 通过、1 失败，实际 Vanished，期望 Done 0），日志 `.autome/output/m03-race-red.txt`；修后绿见验收结果。该机制可以导致观察到的额外执行；没有保留那次失败夹具，无法证明该次失败一定由这一窗口触发。

## 标「待审」之前的自审清单

1. **情形表逐行**
   - Gate：E6 断言未改 watch 的第一次实现不重跑，第二次实现改 lock 后 setup.result 位于两次门禁之间且 node=gate；E7 检查失败原因、无门禁、仅一次实现。
   - MergeGate：E6 验证重新门禁但任务检出准备仍只有 2 次；新增 lib 矩阵验证非复用路径先准备，并验证同 SHA 复用即使准备记录缺失也不启动准备。
   - ReproCheck：E10 定位轮改 lock 后第二次准备成功在复现门禁开始前。
   - FixGate：E10 修复轮再次改 lock，第三次准备成功在修复门禁开始前。
   - MainCheck：E8 的门禁实际依赖 deps/stamp，验证 verify 准备成功事件先于 main_check、门禁成功、无警报，并检查检出和指纹均删除；E9 验证 exit 5 仅报警、门禁跳过、任务 done、检出和指纹删除。
2. **失败分支**
   - 非零退出：四个任务门禁分别运行 exit 9，失败停在原节点、无 gate.started、不写成功记录；verify 使用 exit 5，报警包含命令、退出码和日志路径。
   - 超时：四个任务门禁分别启动真实 sleep 30 wrapper，待 pid 出现后把 started 时间改为过期，确认 124、任务 Failed、无门禁和成功记录。既有 gate 超时测试检查杀进程及准备日志文案。
   - 读盘失败/空输入：矩阵同时使用不存在路径、目录和零字节文件，确认按 absent/空文件规则正常准备；既有 E16 和 domain 指纹测试覆盖空 watch、重复/顺序、absent 与 empty 区分。
   - 拒绝授权、外部服务不可达：不适用独立宿主分支；本功能不请求授权或访问服务，用户 shell 命令返回非零统一由上述失败分支处理，未进行真实联网安装。
3. **全部迁移**
   - 四个任务门禁各执行开始→Pending（原节点、无门禁）→Ready（有门禁）或 Failed（exit 9 / 124，原节点失败）。
   - MainCheck 执行 Pending→Ready→门禁完成→Cleanup，及 Pending→Failed→报警/跳过→Cleanup→Done。
   - MergeGate 同 SHA→复用，新增矩阵直接验证。
   - 共用准备作业的重启接回、停止重跑、暂停恢复、取消、进程消失及运行中输入变化沿用 M-02 已有项目测试，本轮全量重跑；没有新增节点或更改这些控制迁移。
4. **域边界**
   - 本轮不新增配置输入域；缺文件、目录、空文件已在四节点矩阵实际执行。空 watch、同内容不同提交、记录不存在、watch 变化分别由既有 E16、E6 和矩阵覆盖。
   - SHA 取前 12 位沿用现有门禁键规则；正常 git SHA 由真实 git 仓库提供，无新增日期/数字输入。

## 验收命令与结果

- `cargo test -p automed --lib`：最终 616 个通过、0 失败、0 忽略；上一轮基线 614，增加 2（四门禁矩阵 1 个、确定性竞态回归 1 个）。原始输出 `.autome/output/m03-lib-final.txt`。
- `cargo test -p automed --lib setup_exit_published`：修前 0/1，通过修复后 1/1，分别见 `.autome/output/m03-race-red.txt` 与 `m03-race-green.txt`。
- `cargo test -p autome-domain setup`：10 个通过、345 个过滤，较上一轮新增数 0；这 10 个是既有 domain 子集，不累计为新用例。输出 `.autome/output/m03-domain-setup.txt`。
- `cargo fmt --all -- --check` 与 `cargo clippy --all-targets -- -D warnings`：最终通过，输出 `m03-fmt-final.txt`、`m03-clippy-final.txt`。
- `git diff --exit-code 2553923 -- .autome/config.toml` 与 `git diff --check`：通过；未修改仓库准备配置、协议或会话沙箱。
- `cargo test -p automed --test end_to_end`：最终 93 个通过、0 失败、0 忽略；上一轮基线 88，增加 5（E6–E10）。输出 `.autome/output/m03-e2e-final.txt`。
- `cargo test -p automed --test end_to_end setup`：最终 13 个通过、0 失败、0 忽略、80 个过滤；上一轮 8，增加 5（E6–E10）；它是上述 93 个的子集，不重复累计。输出 `.autome/output/m03-setup-verified.txt`。
- 里程碑整 crate 验收合计 709 个（616 lib + 93 e2e），对比上一轮 702 个（614 + 88）增加 7，全部通过。首次 93/93 通过之后，因竞态修复更改了共用轮询代码，才重新执行最终全量。

本轮不执行 M-04 的全工作区/desktop/web/ci-local 验收；真实联网 npm 和界面人工项 H-01/H-02 未取证，仍留人工清单。


结论：M-03 待审；由下一审计轮独立复验。本轮不关闭里程碑。
