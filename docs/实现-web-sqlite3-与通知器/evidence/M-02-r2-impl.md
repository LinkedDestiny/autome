# M-02 实现轮 #2

实现预算 2/35。推进最小开放里程碑 M-02；本轮无审计退回缺陷（M-01 已独立通过），无需搬入上一轮的红灯检查。通知器验收通过；Web 已有监听回归受沙箱限制，未取得全量通过证据。

## 实现与用户裁定

- 增加表驱动的七条规则、中文失败原因/告警渲染与 levelOf；不引入依赖。
- 通知器使用持久游标、单飞分页补拉、补齐缓存、进程内深相等状态记忆、事务写入后推送。离线/超时停在事件之前，成功后才记忆状态；明确 RPC 拒绝生成 NULL 补齐字段的通知。换账本清空状态与缓存。
- 真实 fixture 在临时 HOME/AUTOME_HOME 下用真实 autome 与假 CLI 写账本；CoreSupervisor 经 stdio 读取。project.get 的项目字段位于 project 子对象，已按真实响应读取。
- B-01、B-04 按用户要求追加独立里程碑 M-06、M-07，保持开放。它们分别复用 M-04、M-02 的实现与验收用例，避免复制产品逻辑；本轮已实现 B-04 的补齐失败后同事件重试补步，但不提前关闭其独立里程碑。
- B-02 忽略、不做 W6 角标；B-03 忽略、不持久化最近状态，接受跨重启可能多一条通知。裁定同时记入 retro。

## 命令与结果

长输出均在 `.autome/output/`，以下退出码为命令本身。未修改任何已有测试。

| 命令 | 退出码 | 结果 / 原始输出 |
|---|---|---|
| `npm test --prefix apps/web -- test/notifier-rules.test.ts test/notifier.test.ts test/notifier-real.test.ts` | 0 | 3 文件、25 用例全过，0 跳过；m02-acceptance.txt |
| `npm test --prefix apps/web` | 1 | 27 文件、267 用例：240 通过、11 失败、16 前置失败跳过；m02-web.txt |
| `npm run lint --prefix apps/web` | 0 | 无错误/警告；m02-lint.txt |
| `npm run typecheck --prefix apps/web` | 0 | 通过；m02-typecheck.txt |
| `npm test --prefix apps/desktop` | 0 | 104 用例，103 通过、1 基线跳过；m02-desktop.txt |

上一轮审计基线 Web 242 → 267，净增 25：规则 12、通知器 11、真实账本 2；文件 24 → 27。desktop 104 → 104，差 0。Web gateway-http 的 11 失败均为 `listen EPERM: operation not permitted 127.0.0.1`；server/eventbus-live 前置失败有同一监听错误与 `File watcher error: EMFILE: too many open files, watch`，导致 16 用例未执行。与上一实现轮受限环境表现一致，但不把上一审计的宿主通过当作本轮全量通过。需审计在可监听环境复验，未弱化测试或主动跳过。

## 标待审之前的自审清单

### 1. 情形表逐行

| §3/§4 情形 | 实跑检查与结果 |
|---|---|
| await_design_approval | 原始 seq 21：kind、need_you、中文标题通过；真实计数 3 |
| await_merge | 原始 seq 45：kind、need_you、中文标题通过；真实计数 2 |
| await_info | 原始 seq 124：kind、need_you、中文标题通过；真实计数 2 |
| failed | 原始 seq 116：完整退出码标题通过；真实计数 3 |
| project.alert | 原始 seq 102：标题 main/指定分支/未知 kind；真实计数 1，项目名/任务正文/项目 id 通过 |
| done | 原始 seq 53：fyi 与标题通过；真实计数 2 |
| not_a_bug | 原始 seq 9：need_you 与标题通过；真实计数 1 |
| 表外状态/事件 | draft、queued、paused、stopped、cancelled、其它 active 节点，以及 created/protocol_failure/alert_dismissed 均不通知 |
| 进入与重复 | seq 124→深相等状态（键序不同）→145 locate→150，恰两条通知 |
| 首次/重启/停服补拉 | 首次从 latest_seq、空账本从 0；重启不重，停服期间按序各推一次；真实关库新增事件只增 1 条 |
| 半处理崩溃 | 人工倒退游标后重启被唯一索引挡下，不重推；SQLite 游标触发器失败则通知回滚，重启只补一条 |
| 换账本 | 10→0，旧 seq NULL、日志一行、复用 T-1 后使用新项目 p2 与新正文，正常生成 |
| 补齐/缓存 | task.get/project.get 补齐成功；同 id 只调用一次；明确拒绝继续，离线/超时保留游标重试 |
| 分页/单飞/推送/关闭 | 页大小 2，5 条分页拉完；并发三次 pull 共享 Promise，只补一轮；消息恰五键；close 丢弃在途补齐、不再调用 |

### 2. 设计点名的每条失败分支

- 无 code 的离线错误与带 timeout 标志的超时分别实跑：游标停在待处理事件之前；恢复后同事件产生一条通知、一次推送。B-04 补步已进入项目测试。
- 带 code 的任务不存在/项目不存在：两类分别生成，project_id/body NULL，后续事件继续处理。
- 游标写失败：真实 SQLite 触发器 RAISE，通知与游标原子回滚；解除后重开补齐。
- 空输入：空账本初始化与无新事件空页通过。拒绝浏览器通知授权不适用，本轮不接 Notification API（M-05）。磁盘故障用游标写入异常注入验证，不宣称做了物理磁盘故障实验。

### 3. 全部状态迁移

- 游标缺失→当前 latest（含 0）；有游标→续拉；表外事件→页尾提交；匹配事件→补齐成功→事务提交→推送；匹配事件→暂时失败→原游标→恢复重试；匹配事件→明确拒绝→NULL 补齐→提交；唯一键冲突→推进但不推；提交失败→回滚→重启重做，均有测试。
- 任务进入命中状态→连续同状态抑制→离开至 locate（不通知）→重新进入（通知）已跑；七个匹配入口与全部表外类别分别覆盖。这里不重测内核自身的任务状态机，通知器只消费其事件。
- 账本倒退→重置游标/旧 seq/两类缓存→新事件处理；空闲→拉取→期间再次请求→补一轮→空闲；运行→close（含在途补齐）→不再调用，均通过。
- 无多通道迁移：本轮只通过注入 publish 检查消息，WebSocket 接线属于 M-03。

### 4. 输入域边界

序号 0、单条、缺省初始化查询 Number.MAX_SAFE_INTEGER、超过一页（5 条/页长 2）、空页、无 task_id 的项目告警、已存在唯一键、中文 detail 长度 0/1/60/61 与多种连续空白、未知失败/告警 kind、空 display_name 回退 path 尾段、分支缺省 main 均通过。无日期字符串解析或数字文本输入，前导零、闰日、跨年不适用；日期由 store 生成。

## 负向对照

每次只变异一个行为，用 finally 原样恢复源文件；命令脚本为 `.autome/output/m02-negative.cjs`，原始结果分文件保存。筛选测试的未选中用例不算全量跳过。

| 变异 | 命令（前缀均为 `npm test --prefix apps/web --`） | 预期 |
|---|---|---|
| 补齐失败仍 save(event.seq) | `test/notifier.test.ts -t '补齐无 code'` | 游标断言失败 |
| 补齐之前 states.set | `test/notifier.test.ts -t '补齐无 code'` | 恢复重试通知数量为 0，断言失败 |
| 首次游标写 0 | `test/notifier.test.ts test/notifier-real.test.ts` | 首次启动与真实账本首次初始化断言失败 |
| done 级别改 need_you | `test/notifier-rules.test.ts` | done 原始事件与 levelOf 两例失败 |
| 删除 not_a_bug 规则 | `test/notifier-rules.test.ts test/notifier-real.test.ts` | not_a_bug、七行守卫、真实计数失败 |

五项退出码依次为 1、1、1、1、1（全部符合预期）；见 `m02-negative-summary.txt` 和 `m02-negative-{cursor,memory,initial,level,not-bug}.txt`。删除 not_a_bug 后第二个真实账本用例也因前置通知总数不足而失败，共 4 个失败，不掩盖连带断言。

还原后再次运行同一条三文件验收命令，退出 0，25/25、0 跳过（`m02-restored.txt`）；`git diff --check` 退出 0。已核对状态块为 2/35、M-02 待审，M-01 仍已完成，其余保持开放。无修改已有测试、无残留变异。下一步由 Autome 调度独立审计，宿主需复验 Web 监听测试；本轮不启动下一会话。
