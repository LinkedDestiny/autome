# M-07 实现 #7：人工验收逐项隔离前置

仅推进 M-07（B-2 / D1-N02、C7）。上一轮 M-06 审计通过，无退回缺陷，因此没有需要逐字搬入项目测试体系的审计检查。

## 修改与验收

H-01、H-02、H-04 各自明确在启动 shell 执行 `export AUTOME_HOME=$(mktemp -d)` 和 `unset AUTOMED_DB_PATH`，注册临时仓库，宿主继承隔离环境；H-04 进一步明确面板与创建宿主继承同一环境。删除 H-01 末尾的共同前置，H-02 只复用对话步骤，不再复用隔离说明。使用 export 确保宿主及其子命令能继承变量。

验收命令：`sh .autome/output/impl7/check-manual.sh docs/在-plugins-新增-autome-插件/在-plugins-新增-autome-插件.md`。检查对 H-01、H-02、H-04 分别以 `grep "^- $id "` 提取整条记录，各自用 `grep -Fq` 匹配上述两条命令，两者缺任一即失败。

- 修改前：0/3，通过数为 0，退出 1；H-01 未 export 且使用共同前置，H-02、H-04 缺独立前置。日志 `.autome/output/impl7/manual-before.txt`。
- 修改后：3/3，退出 0；日志 `.autome/output/impl7/manual-after.txt`。这三项为本轮文档契约检查，不增加插件长期用例。
- `sh plugins/autome/tests/run.sh all`：59/59，退出 0，与上一轮 59 项基线差 0。
- `sh plugins/autome/tests/verify-cli.sh readme`：4/4，退出 0，与上一轮 4 项基线差 0。长期检查合计 63 项，差 0，无失败、跳过；加上本轮三条文档检查共 66 项，增加 3 来自逐条隔离前置检查。日志分别为 `.autome/output/impl7/plugin-all.txt`、`readme.txt`。

## 标待审之前的自审清单

1. **情形表逐行**：契约三条均独立提取检查。H-01：临时 AUTOME_HOME + unset AUTOMED_DB_PATH + Claude 继承，通过；H-02：两条前置独立出现 + Codex 继承，通过；H-04：两条前置独立出现 + 面板与宿主共用隔离环境，通过。不依赖其它条目的前置说明。
2. **失败分支**：修改前同一检查已拒绝缺少显式 export 或缺少独立前置的三条记录；修改后全部通过。授权拒绝、读盘失败、超时、空输入、外部服务不可达不适用：本里程碑只改人工验收说明，不执行宿主创建或引入运行时失败分支。
3. **全部迁移**：不适用，没有新增或修改状态机、通道；人工对话的模糊确认与明确确认行为保持原要求。
4. **域边界**：逐条只读单条记录，H-01 的前置不会替 H-02、H-04 通过，修改前结果验证这一边界。数值、日期、字节长度等不适用：本里程碑无此类输入域。

人工宿主验收未执行，符合 M-07 明确约定；以上证据证明前置说明逐项齐备，不代表 Claude、Codex 或浏览器实际创建验收通过。

## 全仓门禁与最终检查

`sh scripts/ci-local.sh` 未通过，汇总仅 Web test 失败，原始输出 `.autome/output/impl7/gate.txt`。Rust 1153 次执行通过；Desktop 104 项（103 通过、1 跳过）；Web 225 项（198 通过、11 失败、16 跳过）。合计 1482 次执行，较上一轮基线差 0；1454 通过、11 失败、17 跳过，均与基线一致。失败仍为 `listen EPERM: operation not permitted 127.0.0.1`，需要可监听环境复验，不能声明全仓门禁绿色；未修改范围外文件或绕过沙箱。

`git diff --check` 通过；状态检查确认 implementation-round 为 7/40，current-milestone 为 M-07，M-07 为待审，M-08 保持开放。仅更新人工记录及本轮协议产物。
