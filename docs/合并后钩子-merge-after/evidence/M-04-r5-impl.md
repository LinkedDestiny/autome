# M-04 实现轮 #5 证据

本轮仅推进 M-04。上一轮审计 #3 对 M-03 结论为通过，没有退回缺陷或需要逐字搬入测试体系的判别检查；红绿复现不适用。文档工作完成，但全量 CI 被运行环境阻塞，M-04 保持开放，不标待审、不增加 reopen。

## 改动与核对

- README 在 gate 示例旁增加 `[merge]`、after、缺省 10 分钟的超时；Preparation 后增加 After merge 条目。逐项核对包含宿主/主检出、MainCheck 后与清理前、准备失败仍执行、两个环境变量、主检出可能包含后续合并、失败仅报警且 Done、MainCheck 失败/跳过及工作区跳过、日志/CLI 输出、gitignore 产物约束、停止后恢复重跑。
- 技术设计 §3.3 增加项目配置示例；§8.1 增加 Cleanup 前置子阶段、作业文件、非阻塞轮询、互斥、prepared、结论事务、重入与生命周期；§9 对照 config_io.rs 核实表/字符串类型、空白、缺省、正整数及 u32 上限、保存与解析规则。
- 没有修改产品代码、测试或本仓库配置。只按协议向 `.autome/output/` 写验证输出。
- 修正验收计数口径：原文把 1219 个登记用例写成 1219 个通过；`crates/autome-domain/tests/t14_gate_logs.rs` 原有 `#[ignore = "requires AUTOME_T14_GATE_DIR pointing to the original T-14 logs"]`，当前该环境变量未设置。预期改为总数 1219、1218 通过、1 原有忽略，并纠正该测试所属 crate；不改变测试或跳过规则。

## 验收命令与结果

原始输出均在 `.autome/output/`，以下只记录结论。

| 命令 | 结果 | 输出 |
|---|---|---|
| `grep -c '\[merge\]' README.md docs/development/technical-design.md` | 退出 0；分别 2、3，且人工逐段核对示例与说明均完整 | 文档正文及 m04-doc-diff.txt |
| `grep -c 'AUTOME_MERGE_COMMIT' README.md` | 退出 0；1 | README After merge 条目 |
| `git diff --exit-code $(git merge-base HEAD main) -- .autome/` | 退出 0；打印 AUTOME-DIR-UNCHANGED | m04-autome-diff.txt（空） |
| `cargo test --workspace --all-targets` | 退出 0；9 个目标，总数 1219，1218 通过、0 失败、1 原有忽略 | m04-all.txt、m04-target-results.txt |
| `sh scripts/ci-local.sh` | 退出 1；用时 429 秒；Format、Clippy、Rust 测试、Contract、Web lint/typecheck/build 通过；Desktop、Web test 未通过 | m04-ci.txt |
| `apps/desktop/node_modules/electron/dist/Electron.app/Contents/MacOS/Electron --version` | 独立启动也退出 134，无输出；支持 Electron 启动环境阻塞的判断，未定位到系统底层原因 | m04-electron-version.txt |
| `git diff --check` | 退出 0 | 无空白错误 |

Rust 各目标：CLI bin 26、CLI integration 13、domain lib 388、domain t14_gate_logs 1（忽略）、automed lib 657、automed bin 0、end_to_end 111、prompt_rendering 16、real_cli 7。CI 内再次得到同样的 1219 总数，不重复累计。

**与上一轮基线的差：**上一轮实跑 automed lib + end_to_end 为 768（657 + 111），本轮同范围仍为 768，差 **0**。M-03 后全工作区预期 1219，本轮总数仍为 1219，差 **0**；与任务起始登记基线 1196 比为 **+23**（domain 4、automed lib 10、e2e 8、CLI 1），日志中名称含 merge_after 的通过用例恰 23 条。本轮新增、删除、改名用例均为 0，没有 rebase 带来的增量。

Desktop 本轮收集 105：104 通过、1 失败、0 跳过；失败是实际 Electron DOM harness 进程 signal SIGABRT、status null，尚未执行该 harness 的 DOM 子检查。Web 本轮收集 301：271 通过、11 失败、19 跳过，33 个文件中 29 通过、4 失败；失败包括 `listen EPERM: operation not permitted 127.0.0.1`，另有文件监听 EMFILE，live suite 初始化失败造成跳过。上一轮未跑 Desktop/Web，没有可比较的实跑基线；不能把收集数当作 DOM 子检查也已完整执行的证明。

## 标待审之前的自审清单

本轮执行了四项核对，但 CI 未全绿，因此不标待审。

1. **情形表逐行：**M-04 落法逐行已核对：README 示例、After merge 条目、技术设计 §3.3/§8.1/§9 均完成；`.autome/` 差异检查通过；Rust 全量通过（保留原有忽略）；全量 CI 未通过，具体环境失败见上。设计 §4 的工作区、无合并提交、MainCheck Failed/Skipped/NotRun/Passed，由全量中的 domain plan、E1/E4/E6/E7、L3 回归通过；README 与技术设计描述一致。
2. **失败分支：**全量中的 E2 钩子非零、L1 超时 124、E4 MainCheck 失败、E5/L5 主检出准备失败、S1 结果事务失败、L2 作业消失、E3 空命令均通过。文档缺少示例或条目的负向条件通过分别阅读两段核对排除；格式/clippy 由 CI 实跑通过。授权拒绝、独立读盘失败、外部服务不可达不适用本轮新增文档：未引入这些接口或降级语义；Web EPERM 是本次验收环境失败，未算作产品正向通过证据。
3. **全部状态迁移：**本轮不新增状态机。全量回归覆盖 Cleanup 的准备 Pending→Ready/Failed→prepared（E5/L5）、互斥等待→启动（L3/L5）、Skip→Done 与重入不重复（E4/E6/L3）、Running→成功/非零/超时→Done（E1/E2/L1/L2）、Vanished→清除→重启（L2）、暂停→恢复接原作业、停止→恢复新作业、取消→终止（E8）、已有结论重入与删除配置（L2）、开始事件缺失补写（L4），全部通过。core 重启沿用设计的直接重入验证，未操作用户驱动。
4. **输入域边界：**全量中的配置用例检查缺表、缺 after、空串/空白、不合法表和命令类型、超时缺省/零/负数/错误类型/最大值截断、未知键与保存保留；domain 检查项目形态/commit 有无/MainVerdict，L3 检查无 pid/活 pid/结束标记，均通过。M-04 无新增输入域；前导零、闰日、跨年不适用，没有新增日期或数值格式。

## 阻塞与下一步

M-04 保持开放；不是协议失败，也没有已证明的产品实现缺陷。当前权限模式不允许提权，不能取得 Electron 正常启动与本机监听的验收环境。下一轮应在允许 Electron 与 loopback 监听的环境运行原样 `sh scripts/ci-local.sh`，取得 Desktop/Web 全部通过证据，再标待审。未改断言、未加 skip、未把环境失败改写为通过。H-01/H-02 仍由用户合并前人工验收，本轮未执行；未运行可选 package。
