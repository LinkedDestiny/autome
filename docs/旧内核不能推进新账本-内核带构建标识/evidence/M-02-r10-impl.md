# 实现 #10：M-02 相关 rebase 冲突处理

## 范围与结论

简报与设计表确认 M-01 至 M-07 均已由审计关闭，没有开放里程碑；上一轮审计 #8 为通过，没有待移植的缺陷检查。本轮执行用户追加的 rebase 指令，不新增里程碑、不重开或重新关闭既有里程碑，保留其审计状态。状态块仅更新 implementation-round 为 10/35，next-action 保持无。

`git rebase main`（main 为 `24fde3fc`，origin/HEAD 指向 origin/main）在重放 `93008274` 时复现 `crates/automed/tests/end_to_end.rs` 两处文本冲突。根因是双方在原文件尾部追加不同测试，Git 把公共括号与测试属性交错配对，并非产品行为缺陷。以 index stage 2 的完整文件为前缀，追加 stage 3 从 `const GUARD_NEW:` 开始的全部内容，随后 `git add`、`GIT_EDITOR=true git rebase --continue` 成功完成剩余提交。没有 merge、切换分支或推送。

解决后以 Node 读取 `git show main:<path>` 和 `git show ORIG_HEAD:<path>` 作逐字比对：当前文件以 main 完整文件开头；GUARD_NEW 起的尾部与 rebase 前完全相同。双方测试无遗漏、无改写。main 的 SCHEMA_VERSION 为 7，本任务为 8，无迁移版本冲突。

## 命令与结果

| 命令 | 用例与结果 | 原始输出 |
|---|---|---|
| `cargo test -p automed --test end_to_end older_kernel` | 7 通过 | `.autome/output/r10-older.txt` |
| `cargo test -p automed --lib kernel_mark` | 3 通过 | `.autome/output/r10-mark.txt` |
| `cargo test -p automed --test end_to_end kernel_adopt` | 3 通过 | `.autome/output/r10-adopt.txt` |
| `cargo test -p automed --lib kernel_adopt` | 1 通过 | `.autome/output/r10-adopt-lib.txt` |
| `cargo test -p automed --test end_to_end integration_e2e` | 4 通过 | `.autome/output/r10-integration.txt` |

总计 **18 用例，18 通过、0 失败、0 忽略**。M-02 同口径对比 r2 审计基线 10 例，差 0；M-03 相关回归对比其基线 4 例，差 0；额外执行 main 的 4 个 integration_e2e 用例，所以本轮比 M-02 基线多 8 例，来自 M-03 的 4 例与集成流程的 4 例，并非新增测试。与最近一轮 M-07 的 8 例相比数值多 10，但测试范围不同，不作为覆盖增长证明。

`git merge-base --is-ancestor main HEAD`、`git diff --check` 均退出 0。rebase 原始输出见 `.autome/output/r10-rebase.txt`、`r10-continue.txt`。未运行宿主快检或全量门禁；人工 H-01 至 H-03 本轮未取证。

## 四项自审清单

1. **情形表逐行**：执行 `kernel_mark_claim_matrix`，覆盖无记录→写入；相同文本→不写；新→旧记录更新；旧→新记录拒绝；同计数不同 SHA→拒绝；git→unknown 更新；unknown→git 拒绝；不同 unknown 拒绝；空/未来格式记录拒绝；unknown 无记录写入及相同 unknown 不写。全部通过。M-02 七条 e2e 分别确认 tick 原句、所有列明写方法拒绝、三个读方法与 env.get 可用、无副作用、未知占位符任务不被旧内核判失败、锁忙跳过可区分、锁错误不推进。
2. **失败分支**：执行旧内核拒绝读写边界、锁目录导致打开失败、DROP TABLE 导致标记读取失败、RESERVED 锁导致 claim 失败、未知占位符正负对照，均通过。集成测试覆盖 rebase 冲突、审计重开、审计缺证据、仅文档修复。外部服务不可达与超时不适用：冲突段没有网络服务或新增超时路径。
3. **全部迁移**：执行 v7→v8 保留任务队列；旧内核拒绝且不恢复会话/固定协议；带逃生口从新标记降为旧标记（tick 与写命令两个入口）；重复接管不追加事件；新内核抬回标记；已接管的旧进程再次被拒，均通过。main 的四条 integration_e2e 全部通过，覆盖冲突修复后的独立审计与后续合并等待流程。
4. **域边界**：矩阵覆盖空记录、无记录、未知格式、前导零、零计数、最大七位计数、闰日与跨年标识；环境测试覆盖缺失、空串、0、true、01、空格、换行及精确的 1；注入身份覆盖无效、相同、同计数不同 SHA 与越界提新，均通过。本轮无新增输入域。

## 提交

冲突修复已随 rebase 写入重放后的提交；本轮证据、轮次与指针另行提交到当前分支。没有触碰真实账本或安装位置，测试沿用项目临时账本夹具。
