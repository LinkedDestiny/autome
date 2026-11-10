# M-05 实现轮 #5

结论：全量运行与实际文件差异检查通过，申请待审；implementation-round 为 5/35。本轮仅推进 M-05，没有修改产品代码或测试。M-06、M-07 保持开放。

## 命令与结果

原始输出都在 `.autome/output/`，测试使用项目现有隔离账本，未打开真实 `~/.autome`。

| 检查 | 结果 | 输出 |
|---|---|---|
| `cargo test --workspace` | 退出 0；985 个用例，985 passed，0 failed，0 ignored | k1-m05.txt |
| `cargo clippy --all-targets -- -D warnings` | 退出 0 | m05-clippy.txt |
| `cargo fmt --all -- --check` | 退出 0 | m05-fmt.txt |
| `cargo run -p automed -- protocol eval protocol/` | 退出 0；8 项通过、0 不通过、1 警告 | m05-protocol.txt |
| `git diff --quiet f20fe14 -- protocol/ apps/ crates/automed/tests/golden/prompts/intake.md`，使用下述临时索引 | 退出 0，受保护文件与基线相同 | 见索引说明 |
| 四个集成测试文件的 `git diff -U0 f20fe14` 删除行计数 | 0，满足设计的 `test … = 0` 判据 | m05-integration.diff |
| §8 既有测试变更复核 | 七条逐条符合；未发现额外修改 | m05-detailed.diff、m05-review.txt |

协议警告是 42 行历史规范没有 CHANGELOG 理由，受保护协议内容与基线完全相同；不是本轮新增问题。宿主 `sh scripts/ci-local.sh` 按任务 C10 留给 Autome 执行，本轮没有宣称该宿主门禁通过。

### 总数与上一轮基线差

- 本轮分项：CLI 单元 17、CLI 集成 8、domain 306、automed lib 577、end_to_end 59、prompt_rendering 11、real_cli 7，共 985；其它二进制与文档测试为 0。
- 上一轮 M-04 没有实跑 workspace，其证据明确记录最近全量为 977，新增 CLI 测试 8 个；本轮相对该最近实跑全量为 +8，全部来自 M-04 的 4 个单元和 4 个集成测试。相对 M-04 完成后推算的 985 为 +0。
- 相对任务原始基线 944 为 +41（domain +7、automed lib +22、e2e +3、prompt +1、CLI 单元 +4、CLI 集成 +4）。本轮新增测试 0，没有整层漏跑。59 个 e2e 中包含原样保留的 56 个既有用例。

### 索引环境偏差与等价执行

会话开始 HEAD 为 `d143a1a`（initial），其父提交 `4099758` 保有项目完整跟踪树；当前索引只跟踪少量引导文件，大量项目文件在磁盘上存在却未被跟踪。这与上一轮证据记录的环境现象相同，不是本轮删除文件。

原索引下的保护检查确实退出 1，原因是 Git 将未跟踪的文件视为基线删除，不能把该结果报告为通过。验收命令按依赖分段执行，测试及三个静态门均使用原工作目录；仅 Git 比较采用临时索引恢复完整跟踪信息：

```sh
GIT_INDEX_FILE="$PWD/.autome/output/m05.index" git read-tree 4099758
export GIT_INDEX_FILE="$PWD/.autome/output/m05.index"
git diff --quiet f20fe14 -- protocol/ apps/ crates/automed/tests/golden/prompts/intake.md
test "$(git diff -U0 f20fe14 -- crates/automed/tests/end_to_end.rs crates/automed/tests/real_cli.rs crates/automed/tests/prompt_rendering.rs crates/autome-cli/tests/cli.rs | grep -c '^-[^-]')" = 0
git diff --quiet 4099758 -- crates/
```

三项均退出 0。临时索引只影响跟踪信息，`git diff <commit>` 比较的仍是当前磁盘内容，并非拿历史提交代替当前工作区。没有修改当前真实索引中的无关文件，也没有恢复或提交 apps/protocol 等目录。完整原命令在原索引下不能通过；以上是针对环境偏差的等价文件内容验证。

## 标待审之前的自审清单

1. **情形表逐行**：M-05 没有独立业务情形表，按其明确要求逐条核对 §8：
   - 1：store 仅将既有 SCHEMA_VERSION 断言 4 改 5；Requeued 原断言不变，包含在 577 个 lib 用例中通过。
   - 2：store 夹具只加 `kind: TaskKind::Feature`；通过。
   - 3：dispatch 三处既有 TaskRecord 夹具只加同一默认字段；通过。
   - 4：scheduler 的 add_task 夹具只加默认 kind；通过。
   - 5：CLI insert 夹具只加默认 kind；通过。
   - 6：launcher 既有 PromptSpec 夹具只加 `card_path: None`；通过。
   - 7：prompt_rendering spec 夹具只加 `card_path: None`；通过。
   - 其余既有集成测试无删除行，原 prompt 金样与 protocol/apps 完全不变；默认创建用例 `create_without_the_new_params_is_unchanged`、原有需求 e2e 均通过。新增 task_card 用例合计 41，均通过。
2. **失败分支**：本里程碑不新增失败分支，复验设计点名的已有分支：未知 kind、空白 brief、超限 brief、标题缺失、非草稿 start、草稿 retro、草稿非法触发器均由全量中的 task_card 用例实际执行并通过；CLI 文件测试实际覆盖读盘失败、非法 UTF-8、空输入、超限、缺 request/brief，reply 测试覆盖缺反馈。拒绝授权、外部服务不可达、超时对 M-05 新行为不适用：本轮没有新增授权、外部服务或超时路径。
3. **全部迁移**：全量执行领域状态机与真实 e2e。Draft + Start → Queued；Draft + Cancel → Cancelled；Draft + SlotAvailable/Pause/Stop/Resume/Approve/Reject/Merge/ExtendBudget/RerunFrom/SessionEnded/CoreStepDone/GateDone 全部拒绝；Queued/Requeued/Active/Paused/Stopped/Failed/Done/Cancelled + Start 全部拒绝。开始后的槽位空闲 → 启动、槽位满 → 排队、草稿取消归档无 worktree、feature/bug 批准设计后 reply → Design，由既有新增用例逐项通过。M-05 自身不引入新状态迁移或通道。
4. **输入域边界**：全量覆盖正文 65536 字节接受、65537 字节拒绝、空串/空白拒绝、中文及占位符原样保存、无一级标题回退与无可用标题拒绝、默认 feature/显式 bug/非法 kind、同秒排队顺序。CLI 文件与 stdin 两个通道均执行。前导零、闰日、跨年不适用：任务卡功能没有日期或数字字符串输入域。

## 修复与审计衔接

上一轮审计 M-04 通过，无实现缺陷退回，因此没有需逐字迁入项目测试的审计判别检查，也不存在本轮应补的红绿修复记录。本轮验收未发现需修复的 clippy、fmt 或产品回归，不为增加测试数量而添加重复测试。

本轮只提交证据文件、设计状态与 M-05 证据指针、retro 一行。真实索引里已有的 .gitignore 改动及其它未跟踪文件保留原状。下一实现轮推进 M-06，M-05 由审计轮独立决定是否关闭。
