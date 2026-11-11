# M-01 · 实现 #2（门禁修复轮）

范围：预算 2/20，仅复现门禁失败；M-01 保持待审，不推进 M-02。起始 HEAD 945853d，产品代码承接 ac5aff9；无上一轮审计文件、无审计判别检查需要搬运。

## 结论与修复说明

原命令 `sh scripts/ci-local.sh` 实际退出 1，用时 634 秒。用户提供的宿主失败在 Test，本轮 Test 全绿，未复现原失败；本轮失败在 Web test，明确报 `listen EPERM: operation not permitted 127.0.0.1`，两个服务启动夹具另报 `EMFILE: too many open files, watch`。宿主原日志 Web 为 225 全通过，当前沙箱运行不能替代宿主复验。未获得原 Test 的具体失败用例及断言，不能推断根因、声称修复或制造红绿证据。

未修改产品代码、既有测试、门禁脚本或沙箱配置。当前权限策略为 never，不能提权解除监听限制；保留完整输出供定位，交由 Autome 在本轮提交上重跑。若宿主 Test 再次失败，下一轮应先读取其具体失败用例和完整断言输出再修复，不推进新里程碑。本轮只更新状态、证据指针和运行记录。

## 命令与数量证据

- `sh scripts/ci-local.sh > .autome/output/m01-r2-ci-red.txt 2>&1`：退出 1。文件名 red 是复现阶段预命名，并不代表复现了原 Test 失败。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint/typecheck/build 全部通过；Web test 失败。依赖目录已存在，安装步骤没有执行；没有使用 package 参数。
- Rust Test：1099 个（CLI 19 + CLI 集成 11 + domain 355 + automed lib 611 + e2e 80 + prompt 16 + real_cli 7），0 失败、0 ignored。相较设计旧基线 1084 为 +15，全部来自上一轮 M-01 的 domain +10、config_io +5；本轮新增 0。相较上一轮已实跑的 domain/lib 966 为 +133，来自这次扩大执行范围的其它 Rust 目标，并非新增用例。
- Rust real_cli 中 6 个用例受 AUTOMED_REAL_CLI 开关控制，当前未设置，提前返回；故框架报告 7 passed 不等于真实模型验收。独立 End to end 再跑 80 个、Adapter flags 再跑 1 个，都是重复检查，不重复计入独立总数。
- Desktop：104 个，103 通过、1 跳过、0 失败；上一轮未跑 Desktop，用户提供尾部也没有该数量，不能给出数值差。
- Web：225 个，198 通过、11 失败、16 跳过。与用户提供宿主基线 225 的总量差 0；本轮无新增/删除测试，失败及跳过来自上述监听/服务启动错误。
- 全量脚本涵盖独立注册用例 1428（1099 + 104 + 225），不是 1428 个实际通过；上一轮无同口径完整基线。计数不包含重复 e2e、Adapter flags 和下述过滤检查。
- `cargo test -p autome-domain setup > .autome/output/m01-r2-domain-setup.txt 2>&1`：10 通过，345 过滤；较上一轮子集 10 差 0。
- `cargo test -p automed --lib setup_section > .autome/output/m01-r2-config-setup.txt 2>&1`：5 通过，606 过滤；较上一轮子集 5 差 0。

## 自审清单（保持待审，不新增报验）

1. **情形表逐行**：本轮重新执行 D1–D10、C1–C5，全部通过。未配 setup/默认 watch 与超时（C4、D10）；完整命令与 watch/超时（C1）；非表（C3）；缺命令或错误类型（C2）；空白命令禁用且忽略其它字段（C2）；watch 非数组、非字符串、空路径、绝对路径、父路径（C3）；非法超时（C3）；保存保留配置（C5）；resolve 传递（D10）；文件内容/路径改变（D1）；命令改变/零文件（D2）；缺失区别空文件（D3）；重排/去重（D4）；无记录/不同/相同及三种检出标签（D5）；失败原因序列化（D8）。宿主记录与调度表不适用：属于未推进的 M-02–M-04。
2. **失败分支**：C2/C3 配置拒绝通过；D6 非零码 7、D9 超时码 124 的失败信息传递通过；D7 非运行态拒绝通过。实际准备读盘失败、授权拒绝、进程超时及外部服务不可达不适用：M-01 只有纯函数和配置解析，尚无宿主执行入口。当前 Web 监听拒绝已由完整门禁真实触发并保留失败，未跳过或弱化断言。
3. **全部迁移**：D6 遍历 22 个节点中 18 个非等待节点 Active × SetupFailed → 同节点 Failed，Action::None；D7 覆盖 22 节点的 Paused/Stopped/Requeued（66 组）及 4 个 Active 等待节点、5 个其它状态，共 75 组拒绝；D9 验证 Bug Locate/ReproCheck/FixGate 分发；D5 无/异/同记录三种判断。全部重新执行通过。
4. **输入域边界**：C2 空串/空白命令；C3 空/空白与单元素 watch、首中尾父路径段、绝对路径、合法 ./ 与中文路径；timeout 0、负数、错误类型拒绝，1/u32::MAX/i64::MAX 接受并按现有规则饱和；D1–D5 零/单/重复文件、空/缺失内容、空记录和 64 位指纹。全部通过。日期/闰日/跨年不适用：没有日期输入域。

## 交接

M-01 保持待审，implementation-round 为 2/20；最终门禁结论以 Autome 对本轮提交的宿主重跑为准。阻塞：原宿主 Test 失败未复现且缺少具体失败正文；本地完整门禁受 Web 网络监听权限和文件监听错误阻塞，不能宣称全绿。
