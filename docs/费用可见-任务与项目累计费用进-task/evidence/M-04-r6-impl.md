# M-04 实现 #6：超限告警与读者文案

## 实现与修复

- 按设计 §4 实现 `budget_crossed`，保留原始费用和，仅判定使用浮点容差。
- 会话回收写入费用后、Onboarding 早退后检查主检出预算；错误只 warn，不改变任务状态和调度。
- `raise_budget_alert` 在同一 SQLite 事务里写警报、去重事件和项目通知事件。去重读历史上限，不读未关闭警报。
- CLI 两种事件输出中文；Web 复用原有规则，只改标题，不增加通知规则；桌面渲染进程、read 白名单未改。
- 上轮 M-03 审计通过，无退回缺陷和待搬入的审计检查；B-07 留给 M-05。本轮编译初检发现 Path/String 与 Value 引用类型不匹配，已按现有 API 修正，最终命令通过。

## 验收命令与结果

完整运行日志：`.autome/output/m04-acceptance.txt`。

```sh
cargo test -p autome-domain --lib budget_alert_ && cargo test -p automed --lib budget_alert_ && cargo test -p automed --test end_to_end budget_alert_ && cargo test -p autome-cli budget_alert_ && npm test --prefix apps/web -- test/notifier-rules.test.ts -t 预算告警
```

总计 **9 个用例通过**：domain 3、automed lib 2、真实脚本/Git/worktree e2e 2、CLI 1、Web 1。相对上一轮该过滤集合基线 0，增加 9；比设计预期 8 多 1，是配置非法/读盘失败不更改状态的 scheduler 检查。Rust 其余用例是过滤掉而非执行通过；Web 定向运行跳过 12 项，随后全文件运行 13/13 通过（原 12 + 新增 1）。未声称工作区全量或宿主门禁通过。

附加检查：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`git diff --check` 通过；`npm test --prefix apps/web -- test/notifier-rules.test.ts` 共 13 项通过。日志分别见 `.autome/output/m04-fmt-check.txt`、`m04-clippy.txt`、`m04-web-full.txt`。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计 §4.1 行 | 执行检查与结果 |
|---|---|
| 从未告警，累计超过 | domain strict 用例 20.25/2000 为真；e2e 0.5/40 写 1 条，任务到 await_merge |
| 等额及浮点尾差 | 20/2000、0.07/7、(0.1+0.2)/30 均不告警 |
| 子美分超额 | 0.071/7 告警 |
| 同上限继续花钱 | domain same/lower 为假；e2e 后续带费用会话不新增事件 |
| 降低上限 | 40/1500，历史 2000，不告警 |
| 提高且越过 | 30.25/3000，历史 2000，再告警 |
| 提高尚未越过 | 25/3000、30/3000 均不告警 |
| dismiss | store 关闭后仍读历史去重；e2e 在设计批准前关闭，后续收费后仍仅 1 条事件、无新打开警报 |

§4.3 文案表：CLI 对预算事件、预算项目警报、旧 main_gate_failed 整行精确断言；Web 标题精确断言，task.budget_exceeded 不匹配规则，原七规则测试通过。

### 2. 失败分支

- 配置非法：把预算写为布尔，check_budget 返回错误且任务状态、事件和警报不变。
- 读盘失败：把 config.toml 替换为目录，检查返回错误，任务状态、事件和警报不变；生产调用点显式捕获错误并 warn 后继续回收。
- 写库失败：删除 events 表，警报写后事件失败，事务回滚无警报；另用触发器仅拒绝 project.alert，前两笔也回滚，两个事件计数均为 0。
- 未配预算：e2e 相同收费步骤仍到 await_merge，预算事件与警报为 0。
- 拒绝授权、超时、外部服务不可达：不适用；预算检查只读本地配置/SQLite，没有授权或远程调用。

### 3. 全部迁移

- 预算提醒状态：未提醒→首次提醒、已提醒→同额/降额不提醒、已提醒→提高但未跨越不提醒、已提醒→提高并跨越再提醒，全部由三个 domain 用例覆盖。
- 警报开启→dismiss→后续收费：e2e 验证持久事件去重；事务成功/第一事件失败/第二事件失败由 store 用例覆盖。
- 任务不新增状态迁移；两条 e2e 均从创建经设计批准、实现、审计、门禁到 await_merge，并断言首次超限后的新增已收费会话存在。Onboarding 保持原早退路径，未新增其预算行为。

### 4. 输入域边界

- 空费用累计 0、最小预算 1 美分的等额与真实子美分超额、最大预算 1e8 美分的等额与真实超额、普通等额浮点尾差均在 strict 用例实跑通过。
- 历史输入 None、相等、较大、较小均覆盖；缺预算由 e2e 覆盖。
- 前导零、闰日、跨年、空串：不适用本里程碑数值判定输入；TOML 数值解析属于已闭合 M-03。

## 范围与后续

M-04 可交独立审计，M-05 仍开放。全工作区门禁、B-07 键集合修正及 CLI 费用列属于 M-05；H-01～H-03 人工验收未取证。未推送、未启动下一个会话。
