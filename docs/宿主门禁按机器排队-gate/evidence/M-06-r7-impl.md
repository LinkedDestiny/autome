# 实现 #7：已关闭里程碑的 rebase 兼容复验

## 范围与状态

本轮简报没有开放里程碑；审计 #6 已关闭 M-01 至 M-06，无退回缺陷。本轮遵照用户追加要求 rebase 到本地默认分支 main（b15e6d05a0212d003fab6c6dc13900bc12626ee0），不新增里程碑、不改写审计关闭结论、不启动下一会话。implementation-round 为 7/30，next-action 为无。H-01 仍待用户合并前确认。

## 冲突处理

`git rebase main` 及三次 `GIT_EDITOR=true git rebase --continue` 完成全部 40 个提交的重放；未使用 merge、未切分支、未推送。`git merge-base --is-ancestor main HEAD` 返回 0。

- `crates/autome-domain/src/config.rs`：ProjectConfig / ResolvedConfig 同时保留主分支 gate_reuse_for_main_check 与本分支 gate_concurrency，各自 serde 属性独立保留。
- `crates/automed/src/config_io.rs`：两个配置键各自解析，保留双方完整测试，不把 concurrency 解析嵌套在 reuse 条件内。
- `crates/automed/src/store.rs`：后续重放发现同位置添加方法，完整保留 last_event_before 与 last_event_of。
- `crates/automed/tests/end_to_end.rs`：完整保留主分支 review scope / main-check reuse 和本分支 queue 用例，分别闭合函数。

原始输出：`.autome/output/rebase-r7.txt`、`rebase-continue-r7.txt`、`rebase-continue2-r7.txt`、`rebase-continue3-r7.txt`；冲突差异 `conflicts-r7.txt`、`conflicts2-r7.txt`、`conflicts3-r7.txt`。这些是工作区临时证据，不提交。

## 验收命令与基线

- `cargo test --workspace`：退出 0，共 1289 个，1288 通过、0 失败、1 个既有 ignored；其中 end_to_end 121/121 通过（263.50 秒），原始输出 `.autome/output/workspace-r7.txt`。
- `cargo test --workspace -- --list`：退出 0，共 1289 个，比审计 #6 的 1262 个增加 27。原始输出 `.autome/output/list-r7.txt`。
- 以 rebase 前 1d88eb7 和重放后源码的 `#[test] fn` 名称集合逐文件对照：新增 CLI render 1、domain config 1、config_io 3、git 3、scheduler 6、end_to_end 9、prompt_rendering 4；合计 27，没有删除测试名。本轮不新建、不删改测试断言；差额均来自 main 的合入。
- `cargo clippy --all-targets -- -D warnings`：退出 0；`.autome/output/clippy-r7.txt`。
- `cargo fmt --all -- --check`：退出 0，无输出；`.autome/output/fmt-r7.txt`。
- `npm test --prefix apps/desktop`：退出 0，105 个，104 通过、1 跳过；对上一轮总数差 0。`.autome/output/desktop-r7.txt`。
- 契约两条 `grep -n "concurrency" README.md` / `grep -n "concurrency" docs/development/technical-design.md` 均退出 0；另以 `rg -n 'concurrency' README.md docs/development/technical-design.md` 汇总：退出 0，两个文件共 5 行命中；`.autome/output/doc-concurrency-r7.txt`。

## 自审四项

1. **情形表逐行**：本轮不推进新里程碑；按 M-06 验收行核对全量 Rust、clippy、fmt、桌面、README、technical-design，结果见上。配置冲突逐项核对默认值、合法值、非法值、缺 command、保存保留和 resolve 取小，沿用 gate_concurrency 系列测试；主分支 reuse 默认/覆盖/非法类型/保存测试均保留并纳入全量。
2. **失败分支**：没有审计退回的判别检查需搬迁或红绿修复。配置非法类型/范围、目录不可读、未知占用、head 不可达退队、暂停作业超时回收均由现有项目测试复验；本轮没有新增授权或外部服务分支，不适用额外授权/外部服务检查。
3. **全部迁移**：本轮未更改状态机。沿用全量 gate_queue 测试核对入队、重复 tick、启动离队、完成释放、暂停/恢复、取消/离开节点、准备与门禁分别排队、重启恢复 FIFO、旧并行作业收尾；主分支 main-check reuse 的执行/复用分支随原测试复验。里程碑全部保持审计关闭状态，不人为重开或重新标完成。
4. **输入域边界**：concurrency 的缺省、1/10、0/11、负数、小数、字符串、布尔、数组、空串、前导零、u32 溢出和项目超过全局均保留在 config/gate_concurrency 用例中；reuse 的 true/false 与非布尔拒绝均保留。日期不是本轮输入域，闰日/跨年检查不适用。

## 最终结果

全量验收通过；上述四项所引用的项目用例均在本次全量中通过。`git diff --check` 与状态字段/里程碑保持检查通过。仅更新本轮证据、设计轮次/证据指针/当前测试总数及 retro 一行，冲突解决已包含在 rebase 重放提交中。无环境阻塞；人工 H-01 未取证。
