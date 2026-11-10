# M-05 实现 #5

## 改动与范围

仅改 `crates/` 产品代码与本任务记录。`task.changes` 只落后时允许合并，逐仓新增 `behind`、顶层新增 `rebase_first`，冲突脏文件仍阻挡；CLI 打印 `merge.auto(active=true)` 与 `merge.rebased`，授权清除不打印。未修改已有测试，未推进 M-06。上一轮审计通过，没有需要逐字迁入的缺陷检查。

## 命令与结果

- `cargo test --workspace > .autome/output/m05-all.txt 2>&1`：退出 0，944 个通过、0 失败、0 忽略。各二进制 13 / 4 / 299 / 555 / 0 / 56 / 10 / 7，doc tests 为 0；相对上一轮基线 942 增加 2（CLI R1 +1、e2e E6 +1），其余数量不变。
- `cargo clippy --all-targets -- -D warnings > .autome/output/m05-clippy.txt 2>&1`、`cargo fmt --all -- --check`：均退出 0。
- `git diff 2cca95b -- crates/automed/tests/end_to_end.rs | grep -c '^-[^-]'`：输出 0；grep 无匹配退出 1 是预期，表示无删除行。
- 首次全量运行新增 E6 末尾断言失败：完成后清理任务分支，面板返回 `available=false`，不能要求存在 `rebase_first`。仅修正新增测试末尾断言，日志 `.autome/output/m05-initial.txt`；产品无需修复。

## 负向对照

保存产品文件到 `.autome/output/m05-{dispatch,render}.saved` 后临时变异：

1. 将 `"mergeable": blocking.is_empty(),` 替换为 `"mergeable": blocking.is_empty() && on_top,`；运行 `cargo test -p automed --test end_to_end the_merge_panel_offers_the_merge_when_only_main_has_moved > .autome/output/m05-e6-red.txt 2>&1`，退出 101，落后时实际 false、预期 true，红。
2. 将事件匹配 `"merge.rebased" =>` 临时改为 `"merge.rebased.disabled" =>`；运行 `cargo test -p autome-cli an_automatic_rebase_is_printed_by_autome_run > .autome/output/m05-r1-red.txt 2>&1`，退出 101，事件返回 None，红。

两处变异均已恢复，最终全量验证原版，E6 与 R1 均绿。此次负向对照是本轮自行构造，不是上一轮退回缺陷。

## 标待审前自审清单

1. **情形表逐行**：E6 检查未落后 `behind=false/rebase_first=false`；只落后 `mergeable=true/blocked_by=null/behind=true/rebase_first=true`；落后且冲突脏文件 `mergeable=false/dirty_worktree`；清理脏文件后按合并到 done，分支清理后 `available=false`。R1 检查授权开启打印默认分支前进、关闭不打印、rebase 事件打印自动 rebase 与 SHA 前 10 位。
2. **失败分支**：冲突脏文件拒绝由 E6 覆盖；两项负向对照均实测红。拒绝授权、读盘失败、超时、外部服务不可达不适用：M-05 没有新增这些路径；现有错误传播保持原样。
3. **全部迁移**：本轮无新增状态机。面板的未落后→落后→落后且脏→清理后合并→完成不可用由 E6 连续执行；事件开启/完成/清除三种打印由 R1 执行。既有自动路径迁移随 workspace 全量复验。
4. **域边界**：R1 覆盖空仓数组、单仓、多仓，SHA 截短逻辑用长度大于 10 的 ASCII 值验证，短 SHA `abc` 保留；E6 覆盖单仓和清理后零仓。日期、数值上限不适用，本轮无此输入域。生产 head 为 Git ASCII SHA。

## 限制

H-01/H-02 真实桌面交互未取证，仍由人工验收；未修改 apps。宿主 `sh scripts/ci-local.sh` 由 Autome 按用户安排执行。
