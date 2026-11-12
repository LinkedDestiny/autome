# M-03 实现 #3

本轮只推进 M-03：Bug FixGate 在 before_merge + quick 下运行 `( 复现 ) && ( quick )`，沿用完整门禁超时；不写 repro-only 标记。收结果与事件继续使用启动种类，MergeGate 仍运行复现 + 完整门禁。

上一轮审计 M-02 通过，没有退回缺陷，也没有需要逐字搬入的审计判别检查。现有测试及断言未修改；新增 4 个库测试与 2 个端到端测试。未推进 M-04/M-05。

## 命令、计数与红绿证据

日志均在 `.autome/output/`。本轮验收命令的唯一用例总数为 12（8 lib + 4 e2e），相对上轮验收 27 为 -15，原因是从 M-02 范围切换为 M-03，并非删减测试。同筛选范围原有基线为 6（4 lib + 2 e2e），本轮新增 6，总数符合设计。
另执行直接相关共享路径回归 11 个（10 gate_quick_host + 1 gate_quick_each_round）；本轮最终验证共 23 个唯一用例，相对上轮 27 为 -4，重复运行不重复计数。

| 命令 | 结果 | 日志 |
|---|---|---|
| `cargo test -p automed --lib -- gate_quick_bug before_merge_fix_gate before_merge_unconfigured` | 8 通过，退出 0 | m03-lib-final.log |
| `cargo test -p automed --test end_to_end -- gate_quick_bug bug_loop_before_merge` | 4 通过，退出 0 | m03-e2e.log |
| `cargo test -p automed --lib -- gate_quick_host` | 10 通过，退出 0 | m03-host-regression.log |
| `cargo test -p automed --test end_to_end -- gate_quick_bug bug_loop_before_merge gate_quick_each_round` | 5 通过，退出 0（含前述 4 个） | m03-e2e-final.log |
| `cargo fmt --all -- --check` | 退出 0 | m03-fmt.log |
| `cargo clippy --all-targets -- -D warnings` | 退出 0 | m03-clippy.log |
| `git diff --check` | 退出 0 | 无输出 |

先加入项目测试、再改产品代码：
- lib 红灯：8 个用例中 5 通过、3 失败，退出 101。新用例抓到 repro-only 仍存在、kind 仍为 repro，见 m03-red.log。
- e2e 红灯：4 个用例中 2 通过、2 失败，退出 101。正常路径没有快检计数文件，失败场景未进入预期修复路径，见 m03-e2e-red.log。
- 实现后上述全部转绿。额外加入真实超时子情形后再次跑最终 lib 验收。
- 负向注入：只移除 `last["kind"] != "quick"`，执行 `cargo test -p automed --lib -- gate_quick_bug_fix_gate_result_never_stands_in_for_the_merge_gate`，1/1 失败，退出 101；实际 gate.reused 为 1，期望 0，见 m03-negative-reuse.log。恢复判断后最终验收该用例通过，未遗留注入。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计 §3 / §7 本轮情形 | 执行检查与结果 |
|---|---|
| FixGate before_merge + quick | runs_repro_and_quick_under_the_full_timeout：启动/结果均 quick，命令准确组合；无 repro-only；完整超时 1 分钟，quick 配置 5 分钟 |
| FixGate each_round + quick | mode_switch_collects_by_launch_kind 的 full 启动行配置 quick=false，复现成功仍得到退出 0，证明未执行 quick；旧 before_merge 测试同时覆盖无 quick 的 each_round |
| FixGate before_merge 无 quick | 原有 4 个库回归、2 个 e2e 原样通过；repro_nodes_export_repro_kind 检查脚本环境与结果均 repro |
| ReproCheck / 无项目门禁的 MergeGate | repro_nodes_export_repro_kind：真实脚本读到 repro，结果命令只包含复现 |
| MergeGate 有完整门禁 | 两条新 e2e 检查复现 + 完整命令、kind=full，完整脚本恰好一行 merge_gate full |
| FixGate 快检在跑，移除 quick / 改 each_round / 改 quick 文本 | mode_switch_collects_by_launch_kind：三种切换分别在退出 0、9 下执行，仍记原组合命令与 quick |
| FixGate full 在跑，改 before_merge + quick | 同上：结果 kind=full；命令保留既有当前配置记账（复现命令），符合设计 B-03，不误记快检 |
| FixGate repro 在跑，加 quick | 同上：仍为 repro，命令为复现命令 |
| 排队/新 head/需求 Gate 切换 | 共享选择分支直接回归 gate_quick_host 全部 10 个，含排队选择、新 head 清理、启动标记及超时 |
| 同 head 且 quick 与 command 文本相同 | result_never_stands_in_for_the_merge_gate：没有 reused，真实启动 MergeGate；负向注入能抓住漏判 |

### 2. 失败分支

- 复现失败：退出 9 时不执行 quick，quick-kind 文件不存在，回 Fix。
- 快检失败：退出 9、126、127、124 均按普通失败回 Fix，结果为原命令与 quick；e2e 检查首次失败 prompt 含命令和 quick-bug-marker，修复后成功。
- 真实超时：真实运行的复现作业等待释放文件；将 started 调到 61 秒前，实际 poll 以完整超时 1 分钟杀掉进程，结果 124、kind quick、日志含“超过 1 分钟”，回 Fix。不是仅用 exit 124 替代超时。
- 启动标记写盘失败及缺失标记：共享 gate_quick_host 标记生命周期/旧作业测试通过，未改变原错误处理；无另一个 Bug 专用读写分支。
- 拒绝授权、外部服务不可达：不适用，本轮无授权或外部客户端接口，shell 非零退出由上述失败路径覆盖。
- 空命令：本轮不改解析；原 before_merge_unconfigured 回归覆盖空完整门禁时仍只跑复现。

### 3. 全部迁移

- Fix → FixGate → FixReview → Rebase → MergeGate → await_merge：正常 e2e 跑通，快检一次、完整门禁一次。
- FixGate 失败 → Fix → FixGate 成功 → FixReview → MergeGate：失败 e2e 跑通，快检两次、完整门禁一次；普通错误、126/127、超时的回 Fix 另有库断言。
- MergeGate 失败 → Fix → 再次复核/合并前门禁：原 bug_loop_before_merge 正常通过；状态机未修改。
- §7 三条 Bug 在途切换加“修改 quick”变体，共 5 组 × 成功/失败 2 种，由模式切换测试逐组执行。
- 同 head 的 quick 结果不能转成合并前 reused；共享 host 的 full 沿用正对照通过。无新增状态机或通道。

### 4. 输入域边界

- 成功 0、普通错误 9、命令不可执行/找不到的 126/127、内核超时 124 均覆盖。
- quick/full 命令同文本 `true`，仍不沿用；quick=`false` 在 each_round 下忽略。
- 最小有效完整超时 1 与 quick 超时 5 区分，启动记录和实际终止均断言。
- kind 的 quick/full/repro、无标记旧作业通过新测试及共享 host 回归覆盖。
- 空 quick、类型错误、最大分钟值等配置域不适用本轮变更（由 M-01 解析与 M-02 标记管理）；日期、闰日、跨年、前导零不适用，没有对应输入。

## 边界与交付

未运行 `sh scripts/ci-local.sh` 或其它项目门禁。仅改当前 worktree 的 scheduler、端到端测试及本轮记录；`.autome/` 下仅按协议写 output 日志/临时验证文件，未改配置、规则、启动脚本。未改旧测试断言，未推送远端，不启动下一会话。
