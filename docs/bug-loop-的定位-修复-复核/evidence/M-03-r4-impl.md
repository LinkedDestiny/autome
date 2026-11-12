# M-03 r4 实现证据

结论：rebase main 完成，集成验收通过，提交独立审计。implementation-round: 4/10。

## 范围与处理

上一轮审计通过 M-02，无退回缺陷或待搬入的审计回归检查。开场两个里程碑均已关闭，
因此按用户追加的 rebase 要求新增唯一 M-03，保留 M-01/M-02 审计结论，不自行关闭里程碑。

默认分支 origin/HEAD 指向 main，本地 main 为 9626bc1。
执行 git rebase main，六个指定文件均发生冲突，逐一解决后 git add 与
GIT_EDITOR=true git rebase --continue，22 个步骤全部完成，没有 merge、切分支或推送。
git merge-base --is-ancestor main HEAD 返回 0。日志：.autome/output/r4-rebase.txt、
r4-conflicts.txt、r4-continue.txt。

- launcher：输入槽切分与 main 的 render_gate_parts 组合；先选门禁模式，再填模板及数据，保留旧模板复核提醒。
- eval 测试：夹具及查询均采用 v999，避免与实际协议版本撞名。
- prompt_rendering：保留 main 的门禁金样及 brief 检查和本任务三份输入金样。
- CHANGELOG：完整保留 main v3，三条 I-* 以 v4 追加；当前版本断言同步 v4。
- fix / fix-review：两个门禁分支正文都加 inputs，保留上游边界说明与门禁策略。
- 新交叉测试 bug_inputs_and_gate_modes_preserve_user_data_independently：
  2 模式 × 3 步骤 × 4 输入组合，共 24 组断言，计作 1 个项目测试用例。

## 红绿与验收结果

首次集成把 inputs 放在门禁标记前，违反标记独占行约束：
cargo test -p automed --lib launcher：总数 70，61 通过、9 失败。
将 inputs 移到两个分支正文开头后同命令 70/70 通过。
原始记录 r4-launcher.txt / r4-launcher-green.txt，未改标记解析规则或弱化测试。

首次金样检查总数 27，26 通过、1 失败：fix-review-inputs 缺 main 的写入边界段。
确认差异只有这两行，执行
AUTOME_UPDATE_GOLDEN=1 cargo test -p automed --test prompt_rendering bug_loop_fix_review_prompt_with_inputs_matches_its_golden
更新该金样，再加入交叉测试，全套 28/28 通过。
记录 r4-golden.txt / r4-update-golden.txt / r4-golden-green.txt，旧金样未更新。

以下日志均在 .autome/output/，均退出 0：
- cargo test -p automed --lib launcher：70；上一轮 62，+8，来自 main 新增门禁相关测试（r4-launcher-green.txt）。
- cargo test -p automed --test prompt_rendering：28；上一轮 19，+9，其中 main +8、本轮交叉测试 +1（r4-golden-green.txt）。
- cargo test -p automed --lib protocol::：67；原验收基线 61，+6，来自 main 协议及门禁检查（r4-protocol.txt）。
- cargo test -p automed --test end_to_end bug_loop：18；上一轮 15，+3，来自 main 门禁/队列测试（r4-e2e.txt）。
- 上述四组总数 183，对应前轮基线 157，差 +26（main +25、本轮 +1）；0 失败、0 忽略。
- cargo run -p automed -- protocol eval protocol/：8 通过、0 不通过、1 既有警告，与上一轮检查项数相同（r4-eval.txt）。
- cargo fmt --all -- --check：通过（r4-fmt-check.txt）。
- sh plugins/autome/tests/run.sh evals：5/5，与基线相同（r4-plugin.txt）。
- git diff --check：通过；对 main 检查 plugins、.autome、apps、scheduler、dispatch、CLI 无净差异
  （r4-excluded.txt）。本任务协议副本对 rebase 前 ORIG_HEAD 无差异（r4-frozen-protocol.txt）；
  副本对 main 是任务初始化新增，不能把这些新增文件误当本轮协议修改。

未跑 cargo test --workspace 和 ci-local.sh；本轮只验收冲突关联子集，全量门禁由合并前宿主执行。
旧 df489780fe 的门禁不作为当前提交证据；未跑付费行为 eval。

## 标待审之前的自审清单

1. 情形表逐行：三步骤 × 两门禁模式下，无输入、仅附件、仅参考、两者都有共 24 组全部通过。
   有输入标题各一次，附件为副本相对路径、不泄漏源路径；无输入既有金样逐字节通过。
   each_round 保留原门禁文案，before_merge 保留合并前门禁文案，用户命令原样输出。
2. 失败分支：门禁标记非独占行已实际红、修后绿，静态门同时通过；
   before_merge_prompt_legacy_bug_review_gets_one_note 通过，旧协议兜底仍保留。
   拒绝授权、读盘失败、超时、外部服务不可达不适用：本轮只组合内存中的模板和字符串，不改这些 I/O 分支。
3. 全部迁移：没有新增状态迁移，状态机不适用；两个门禁通道 × 三步骤全部跑过，
   现有 Bug Loop e2e 18 个通过，未改变预算或状态守卫。
4. 输入域边界：空列表、单元素、绝对路径、含空格文件名、文件名/参考/需求/门禁命令带花括号均通过；
   多附件由既有 launcher 和三份输入金样覆盖。日期、闰日、数值最大值不适用：输入是路径与文本。

四项完成后将 M-03 从开放改为待审。下一步由独立审计复验，本轮不启动下一会话。
