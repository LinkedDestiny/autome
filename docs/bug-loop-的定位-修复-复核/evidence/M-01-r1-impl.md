# M-01 · 实现 #1

结论：验收通过，报待审。无上一轮审计，无需要迁入的审计回归检查。本轮仅推进 M-01。

## 修复说明

共用 render_inputs 拼装标题与列表；整理轮传原值，Bug 侧取 file_name，取不到时回退原值。三模板行首紧贴 inputs；先切 request / inputs 再检查模板，最后插入用户文本。新增五个 launcher 单测、三个带输入金样测试与金样。既有十二份金样无差异。协议 v3 增加三条 clarify，版本断言同步。

设计遗漏了 eval.rs 测试夹具固定追加 v3：与真实 v3 重名，changed_cases 取到真实条目。原断言在 `.autome/output/m01-acceptance.txt` 为红（60 通过、1 失败）；仅将夹具和查询改为 v999，保留原断言，最终 61 全绿。设计清单同步，eval 生产逻辑未改。

## 验收命令与结果

```sh
cargo test -p automed --lib launcher && cargo test -p automed --test prompt_rendering && cargo test -p automed --lib protocol:: && cargo run -p automed -- protocol eval protocol/ && git diff --name-status 83c80e9 -- crates/automed/tests/golden/prompts/ protocol/
```

退出码 0，原始输出 `.autome/output/m01-acceptance-final.txt`。

- launcher 总数 62，基线 57，增加五个设计指定输入单测；全部通过。
- prompt_rendering 总数 19，基线 16，增加三个角色带输入金样测试；全部通过。
- protocol 总数 61，基线 61，差 0；全部通过。三组共 142，基线 134，增加 8，无失败或忽略。
- 静态门 8 通过、0 不通过、1 既有警告；CHANGELOG 8 条，基线 5，新增三条 clarify。
- 范围 diff 恰好七行：三份新增金样、CHANGELOG、三个 Bug 模板。受保护范围 diff 为空；git diff --check 和 cargo fmt --all --check 通过。
- 金样先按既有金样加预期输入段构造，19 项比对通过（m01-golden-preview.txt），再用 `AUTOME_UPDATE_GOLDEN=1 cargo test -p automed --test prompt_rendering` 接受（m01-golden-accept.txt），最终无更新模式复验通过。

## 标待审之前的自审清单

1. **情形表逐行**：Locate、Fix、FixReview 各执行附件+参考、无输入、仅附件、仅参考、花括号五类检查，全部通过。模板位置由既有四份 Bug 金样和新增三份金样验证。整理轮原值专项单测及既有需求金样通过。v3 与三条 CHANGELOG 条目通过协议检查。
2. **失败分支**：三个步骤空输入无标题、残留占位符、额外空段；既有 bug_loop_bug_prompts_leave_no_placeholder 逐步检查缺预算、缺模板均拒绝。用户花括号通过，模板未知占位符既有拒绝测试通过。拒绝授权、读盘失败、超时、外部服务不可达不适用：本里程碑为内存纯渲染，没有此类 I/O。空串和根目录附件回退断言通过。
3. **全部迁移**：状态迁移不适用，本轮不改状态机；三个渲染通道全部覆盖上述组合，有任务卡定位入口由 locate-card 金样覆盖。端到端状态迁移属于 M-02，本轮未执行。
4. **域边界**：空列表、单元素、多附件、绝对路径、空格文件名、花括号、空需求均执行通过；预算 0/1、1/1、20/20、u32::MAX 由既有测试循环验证通过。日期、闰日、跨年不适用，输入无日期域。

## 负向对照

临时变更全部还原，最终验收在还原后的代码上执行。日志均在 `.autome/output/`：

- Bug 附件不取文件名：列表单测失败（本轮工具输出），三份新金样全失败（m01-negative-bug-path.txt）。
- 整理轮也取文件名：原值单测失败（m01-negative-intake-path.txt）。
- inputs 独占一行：19 项中 7 失败，含既有 locate、locate-card、fix、fix-review 及新增三份（m01-negative-blank-line.txt）。
- 插入 inputs 后再检查占位符：花括号单测失败（m01-negative-braces.txt）。
- 移除 v3：版本断言失败（m01-negative-version.txt）。

M-02 的 e2e、workspace 全量、ci-local、插件 eval 本轮未跑，留给下一里程碑。
