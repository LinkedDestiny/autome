# M-04 · 实现 #5

范围：仅 M-04（合并后主检出准备、工作区跳过、文档与全量回归）。上一轮 M-03 审计通过，无退回缺陷，故没有须逐字迁入的审计检查。

## 实现

- Cleanup 前调用已有 ensure(main)，Pending 留在 Cleanup；成功按原流程清理，非零退出/超时写 setup_failed 项目警报后照常 Done，失败不写 main.fp。
- 主检出记录跨任务共享。需要重跑时扫描其它任务的 setup-main 作业，复用 gate::running，包含 started 已写、pid 未写的启动空窗；已有 exit 的未收集作业不再占用。
- 会话前、门禁前、Cleanup 三处对已配置 setup 的工作区去重写 setup.skipped，不运行命令。
- README 添加配置、运行/失败语义及 npm ci 保持 lock 文件的提醒；技术设计 §8.1/§9 同步运行机制与解析规则。

## 开发检查与修正

- 初次编译发现 skip_workspace 返回错误需要显式转换为 SchedulerError，以及测试使用了不存在的 update_task；改用已有错误转换和 set_task_state。
- 首次过滤测试中 L3 和 E13 的夹具读取不存在的 config.toml 失败；改用项目现有 configure 的 read_to_string(...).unwrap_or_default() 写法，未修改旧测试或产品规则。
- 修正后过滤测试 lib 11/11、e2e 15/16；唯一失败为尚未修正的 E13 夹具读盘，原始记录 `.autome/output/m04-focused.txt`。E13 修正后独立运行 1/1，记录 `.autome/output/m04-workspace.txt`。最终验收以下列完整门禁为准。

## 标待审之前的自审清单

1. **情形表逐行**
   - 无 gate 也执行主检出准备：E11 三个真实任务均完成合并与清理；A 无记录准备 1 次，B 内容相同 0 次，C 修改 watch 准备 1 次。shell 实际 cwd 日志与各任务 checkout=main 的成功事件双重计数，三次主检出均干净。
   - 主检出准备失败：E12 shell 在 main 退出 4，断言 Done、任务 worktree 与分支被删除、一条 setup_failed 警报含命令/4/日志、main.fp 不存在、失败 result 事件存在。
   - 工作区全部跳过：E13 用 Workspace 真实多仓夹具走到 Done，多次会话和 Cleanup 后 skipped 恰 1、started 为 0、命令目标文件不存在。
   - 跨任务互斥：L3 分别构造 started 无 pid、存活 sleep pid、杀掉并写 exit 三段；前两段 Cleanup 保持且 started 为 0，第三段开始准备，最终 Done。第三段的另一任务尚未收集结果，验证不把它继续当作占用。
   - 无 setup：全量保留所有现有测试；既有 E5 断言没有任何 setup.* 事件。
2. **失败分支**
   - 非零退出：E12 的退出码 4、警报和清理断言。
   - 超时：L3 在第二组用真实 sleep wrapper，等 pid 发布后将 started 调到过去，轮询得到 124；断言 Done、一条含退出码 124 的警报、无 main.fp。
   - 读盘失败/空输入：L3 超时组 watch 包含 missing、directory、empty，分别覆盖不存在、目录无法当文件读取、零字节文件；成功组省略 watch，覆盖空列表首次准备。既有指纹及配置测试覆盖其比较语义。
   - 拒绝授权、外部服务不可达：不适用独立分支，准备由宿主直接执行 shell，无授权/服务协议；其非零退出统一走 E12 已验证路径，真实联网 npm 验收仍为 H-01。
3. **全部迁移**
   - Cleanup → Pending（其它任务启动空窗/存活作业、本任务新启动与运行中）→ Cleanup：L3。
   - Cleanup → Ready（首次成功、指纹命中、变化后成功）→ Done：L3、E11。
   - Cleanup → Failed（非零/超时）→ 项目报警 → Done：E12、L3。
   - 工作区 Cleanup → 跳过 → Done：E13；无配置沿用原清理由全量回归覆盖。
   - 不新增 domain 状态迁移；暂停/停止/重启继续使用既有作业机制，已有 setup 停止恢复与 core 重启 e2e 随全量执行。
4. **输入域边界**
   - 无成功记录、相同指纹、变化指纹、单个 watch：E11。
   - watch 缺省空列表、缺失文件、目录、空文件：L3（空列表成功；其余被读取后正常启动并超时收尾）。
   - 项目类型边界：单仓 E11/E12、多仓 E13；其它任务 started-only/alive/exited 三态 L3。
   - 日期、前导零、最大数值：本里程碑无这类新增输入；配置超时正整数边界由既有 M-01 测试覆盖，本轮无解析改动。

## 最终验收

- `sh scripts/ci-local.sh` 中的 `cargo test --workspace --all-targets`：1122 个通过、0 失败、0 忽略。组成：domain 355、automed lib 617、end_to_end 96、prompt_rendering 16、real_cli 7、CLI bin 20、CLI integration 11。
- 上一轮总数 1118（本轮前 lib 616、e2e 93，其余不变），本轮 +4：E11/E12/E13 三个 e2e，L3 一个 lib（内部含成功/超时两组）。相对任务原始基线 1084 为 +38。automed lib + e2e 共 713，对比上一轮 709 为 +4。CI 单独重复的 96 个 e2e 和 Adapter flags 子集不再加入唯一总数。
- Format、Clippy 均通过。全量 Rust 日志及后续 Desktop/Web 结果在 `.autome/output/m04-ci.txt`。
- `rg -c '\[setup\]' README.md docs/development/technical-design.md`：各 2 处。`git diff --exit-code 2553923 -- .autome/config.toml` 返回 0，CONFIG-UNCHANGED。
- CI 的独立 `cargo test --test end_to_end -- --nocapture` 同样 96/96 通过；Adapter flags 1/1 通过。
- Desktop 总数 104：103 通过、0 失败、1 跳过（Electron 二进制缺失且下载 fetch failed）。Web 总数 225：198 通过、11 失败、16 跳过，22 文件中 19 通过、3 失败。前端测试源码未改，本轮新增用例为 0；上一轮没有 Desktop/Web 实跑计数，无法给出与上一轮实测数量差，不将其当作零跳过通过。
- Web 失败来自三个需要本机服务的文件：gateway-http 的 11 项为 listen EPERM；server 的 9 项和 eventbus-live 的 7 项在 beforeAll 启动失败后跳过，日志含 EMFILE watcher 错误以及 listen EPERM。
- 独立最小检查：`node -e 'const s=require("node:net").createServer();s.once("error",e=>{console.error(e.code,e.message);process.exitCode=1});s.listen(0,"127.0.0.1",()=>{console.log("LISTEN_OK");s.close()})'` 返回 1，输出 `EPERM listen EPERM: operation not permitted 127.0.0.1`，记录 `.autome/output/m04-listen-check.txt`。它不加载项目代码，证明当前会话的监听权限阻塞。
- CI 退出码 1、耗时 498 秒；Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web typecheck、Web build 步骤通过，只有 Web test 步骤失败。未跑 package（不在 M-04 验收要求内）。
- 没有更改/削弱任何旧测试，也没有尝试绕过沙箱或改动会话权限。M-04 实现与新增 Rust 验收已完成，但未取得要求的 `ci exit 0`，因此保持 **开放**，不标待审；不是协议失败。下轮在允许 loopback 的宿主环境执行同一 CI，人工验收项 H-01/H-02 仍保留。


