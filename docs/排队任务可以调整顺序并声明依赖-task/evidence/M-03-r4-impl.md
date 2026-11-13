# M-03 · 实现 #4 · 快检修复

本轮为宿主快检修复轮，不推进新里程碑，M-03 保持待审，implementation-round 为 4/30。上一轮审计仅复验 M-02 并通过，没有待迁入的缺陷检查。

## 修复与红绿证据

宿主在 d2cb808d5c 上发现 `clippy::cloned_ref_to_slice_refs`。检查 dispatch 测试同类位置，三处均位于 `after_blocked_set_rejects_without_side_effects`：把预期单元素数组 `[a.clone()]` 改为 `std::slice::from_ref(&a)`，保留原有拒绝、依赖不变和事件不变断言。未改产品逻辑或用例数量。现有项目 Clippy 检查即为回归检查，不另加镜像测试。

原样运行快检：

```sh
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web
```

- 修复前：退出 101，原样复现三处 Clippy 错误。日志 `.autome/output/m03-r4-quick-red.txt`。
- 替换后首次检查：格式检查要求多行排版，退出 1。日志 `.autome/output/m03-r4-quick-green.txt`。
- 执行 `cargo fmt --all` 后再次原样运行：退出 0，fmt、Clippy、Web lint、Web typecheck 全部通过。日志 `.autome/output/m03-r4-quick-final.txt`。最终快检结论以 Autome 提交后重跑为准。

## 四项自审清单

1. **情形表逐行**：本轮仅修复测试表达式；逐项核对三处对应情形：非法依赖输入拒绝后原依赖不变；Implement/Done/Cancelled 拒绝后原依赖不变；有实现历史的 Stopped Implement/Active Design 拒绝后原依赖不变。均保留原断言并由 `after_blocked_set_rejects_without_side_effects` 实跑。
2. **失败分支**：上述用例覆盖自引用、环、跨项目、不存在、空字符串、前导零、超限、错误类型、缺少参数及不可编辑状态，拒绝信息与无副作用断言均保留。授权、读盘、超时、外部服务不可达不适用：本轮未新增这些分支。
3. **全部迁移**：本轮没有产品状态迁移变更；M-03 原验收子集重跑通知去重、失败恢复再通知、清空依赖启动、设计期与实现期 Requeued 区别等迁移。未把上一轮域层全状态表执行结果算作本轮证据。
4. **输入域边界**：改动所在用例实跑空字符串、前导零、9 项、缺失/null/错误类型；同组接受测试覆盖空数组、单元素、8 项和重复项。日期边界不适用：接口无日期输入。

## 验收与计数

命令：`cargo test -p automed after_blocked_ && cargo test -p automed after_create_`。

退出 0：after_blocked 7 个（lib 6 + e2e 1），after_create 6 个（lib 5 + e2e 1），共 **13 个不同用例，全部通过**，失败 0、忽略 0。与上一轮相同里程碑验收基线 13 个相比 **差 0**；与上一轮含域层的总执行范围 25 个相比 **少 12 个**，原因是本轮未改域层，未重跑 deps::，没有删除或跳过任何所选测试。快检为静态检查，不计入用例总数。

原始输出：`.autome/output/m03-r4-tests.txt`。`git diff --check` 通过。未跑 `sh scripts/ci-local.sh`，由宿主合并前执行；未启动下一会话，未访问真实账本。
