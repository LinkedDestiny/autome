# M-02 · 实现 #3

## 标待审前的自审清单

以下四项在标待审前逐项执行，以本轮最终验收与负向对照结果为据。

1. 情形表逐行：集成审计表不变/退回 × 写入/不写入指定证据；预写同名与旧轮证据；关闭里程碑指针替换/新增；开放里程碑、围栏、无基线；版本门、后续实现轮和审计重试，逐行对照设计 §4、§7、§8。
2. 失败分支：无证据时不变表报未推进、退回表报缺证据；基线缺失/对象不可读时不猜；无范围事件/旧模板不启用。拒绝授权、超时、外部服务不可达不适用：本里程碑无此类调用，范围读取失败属于已关闭的 M-01。
3. 全部迁移：集成审计保持 Done / 退回 Open，各配有/无证据；实现重试共用首次实现 id；审计崩溃重试及审计收尾 task.head 后重跑沿用首次证据状态；新实现轮使范围失效；旧协议走原路径。
4. 输入域边界：空文档、无范围、无基线、缺失证据、单里程碑、行首列表前缀、两种代码围栏与不同围栏长度、无归属标题、非独占标记、状态块轮数与事件轮数不同、文件同内容重写/删除/新建。日期、闰日、跨年、前导零数值不适用：本里程碑不引入日期或数字文本解析。

## 修复与范围

- 新增两条纯函数守卫：集成审计必须在审计期间写指定证据；实现轮不得新增或改写已完成里程碑指向 `-impl.md` 的最新证据行。
- 调度器在审计启动时写入 integration 与证据状态，收尾将指定路径的当前状态交给守卫；指针基线取上一条 task.head 的 Git 对象。
- 集成范围选择按最近实现尝试组定位；审计重试保留首次证据状态；固定模板缺少独占标记时不启用。
- 本轮只实现 M-02。简报/prompt 的渲染属 M-03，协议发布属 M-05，均未推进。
- 上轮审计 M-01 通过，无实现缺陷判别检查需要逐字搬入。既有 24 个 guards 用例断言不变，仅为 Round 增加空字段。

## 验证结果

### 命令、数量与基线

| 命令 | 用例总数 | 上轮基线与差 | 结果与原始输出 |
|---|---:|---|---|
| `cargo test -p automed --lib guards` | 34 | 24 → 34，+10：6 个集成证据守卫、4 个指针守卫 | 34 通过，0 失败/忽略；`.autome/output/m02-final-guards.txt` |
| `cargo test -p automed --lib integration_wiring` | 5 | 0 → 5，+5：证据状态重试、真实启动与收尾、旧范围失效、旧模板、Git 设计基线 | 5 通过，0 失败/忽略；`.autome/output/m02-final-wiring.txt` |
| `cargo test -p automed --lib integration_range` | 17 | 上轮 M-01 为 17，本轮差 0；修改 integration/scheduler 后的直接相关回归 | 17 通过，0 失败/忽略；`.autome/output/m02-final-range.txt` |

验收命令按契约以 `&&` 串行运行，退出码 0。M-02 验收合计 39 个，对相同范围基线 24 个增加 15；加直接相关回归，本轮共 56 个独立用例，对对应基线 41 个增加 15。负向对照是对其中 4 个用例的重复运行，不计新增用例。基线实跑输出为 `.autome/output/m02-baseline.txt`；上轮 M-01 的 17 个见 `M-01-r2-audit.md`。编译期间修正了 Store 类型路径、闭包借用和测试状态的 Box 构造，最终无编译错误。

仅对四个改动 Rust 文件执行 `rustfmt --edition 2024 --check`，退出码 0；`git diff --check` 退出码 0。未执行项目门禁、全量测试、全项目 fmt/clippy、Web lint/typecheck，按本轮指令交宿主执行。

### 四项自审的逐行结果

| 清单项 | 情形与检查 | 执行结果 |
|---|---|---|
| 情形表 | 设计 §7：不变表无文件、不变表预写同名文件、不变表只含旧轮文件 | 三个对应 `integration_audit_*` 用例均报 `audit_made_no_progress`；detail 含范围、指定路径与退回方法 |
| 情形表 | 设计 §7：不变表写证据、退回表写证据、退回表无证据 | 前两项通过；后一项仅报 `evidence_missing` |
| 情形表 | 设计 §8：替换或新增已完成指针 | 两个 `closed_pointer_*` 用例报 `impl_closed_milestone`，真实调度器基线测试同样拦截 |
| 情形表 | 设计 §8：开放里程碑、原有 impl 指针不变、换另一 audit 指针、围栏样例、无归属 | 对应指针用例均不误报 |
| 情形表 | 设计 §4：匹配当前尝试组且模板有标记、后续实现使旧范围失效、旧模板 | 三个 wiring 检查均符合预期；真实启动把 integration 写入 session.started，收尾确实传给守卫 |
| 失败分支 | 缺证据两种失败；无 task.head / Git 对象不存在；无范围事件 / 无独占标记 | guards 与 wiring 全部覆盖：应拒绝的拒绝，设计指定不猜的返回 None |
| 全部迁移 | Done→Done 与 Done→Open，各配有/无证据 | 四项均跑过；本轮只验证守卫接线，退回后完整调度流程属于 M-04 |
| 全部迁移 | 实现崩溃后重试；首次审计→重试→审计收尾 task.head 后重跑；新的实现轮 | 首次实现 id 与首次审计证据状态保留，已写证据继续有效；新实现使旧范围失效 |
| 域边界 | 空串、单元素、缺文件、轮数 99 与事件轮数 3、列表前缀 `- ` / `* `、四反引号内三反引号、波浪线围栏、普通标题重置归属、非独占标记 | 相关 guards/wiring 断言全部通过 |
| 域边界 | 预写文件、同内容重写、新建、删除 | 真实文件检查：预写不算；用确定性旧 mtime 后同内容重写算本轮写入；新建算；删除不算；无 sleep |

第 2、4 项中未适用的外部服务、日期等边界及理由见开头清单。全部四项已完成，无略项。

### 负向对照（改坏为红，恢复为绿）

每项仅临时注入一个设计点名的错误，执行下列命令后立即恢复源文件。四次均为断言失败（退出码 101，1 个用例失败），不是编译失败；恢复后上述最终验收全部绿。

| 注入错误 | 判别命令 | 红色输出 |
|---|---|---|
| 忽略 integration，恢复全完成表免查 | `cargo test -p automed --lib integration_audit_with_a_still_table_and_no_evidence_made_no_progress` | `.autome/output/m02-negative-exemption.txt` |
| 指针检查同时检查轮前 Open | `cargo test -p automed --lib closed_pointer_rules_leave_open_milestones_and_fences_alone` | `.autome/output/m02-negative-open-pointer.txt` |
| 不比对最近实现尝试组 id | `cargo test -p automed --lib integration_wiring_a_later_impl_session_supersedes_the_range` | `.autome/output/m02-negative-stale-range.txt` |
| 把已有同名文件当作本轮证据 | `cargo test -p automed --lib integration_audit_with_a_prewritten_evidence_file_and_no_new_write_made_no_progress` | `.autome/output/m02-negative-prewritten.txt` |

## 结论与交接

M-02 验收通过，可标待审，等待独立审计；没有将任何里程碑标成已完成。`implementation-round` 固定为 `3/25`。尚未实现的 M-03 会把本轮选择出的 integration 接到简报和 prompt 渲染；当前源协议没有启用标记，正常任务仍走旧路径。证据状态按设计采用存在性/mtime/长度，单纯 touch 可算写入的既有风险 R2/B-03 未扩大处理。本轮未 rebase、未改协议与既有金样/e2e、未启动后续会话。

