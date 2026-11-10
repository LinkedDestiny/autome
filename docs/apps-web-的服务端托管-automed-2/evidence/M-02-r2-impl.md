# M-02 · 实现 #2

结论：读写门禁迁移完成，专项 46 个用例通过；提交独立审计。全量 Web 的 7 个 W1 监听用例因本轮沙箱 EPERM/EMFILE 未执行，不能宣称全量验收通过。按设计「风险与验证安排」第 2 项记录后交审计轮复验。本轮仅推进 M-02。

## 改动与迁移

- 新增 `apps/web/server/gates/{read-gate,write-gate}.ts`，两份独立纯函数，无相互或 desktop 导入。读表 13 项、写表和角色/长度限制沿用 desktop；不改 desktop、内核、依赖和 CI。
- CommonJS 改为 ESM，入参 `unknown`，运行时检查后收窄类型；表加只读类型，保留运行时冻结。消息中文化，原来以 `.length` 衡量的限制仍为字符数，错误消息不再误称 byte。
- 读参数字符串增加原 `looksLikeAPath` 检查；四个内部读方法不可达。写附件仅接受小写 UUID 数组，缺省/空数组通过；非空或非数组 doc_refs 拒绝。固定文档枚举保留。COMPONENTS 独立导出供校验及内核对照。
- 原 ipc-gate #1–8 迁到 read-gate；#9 按设计留到 M-04 Origin 测试；#10 移到 write-gate，labels.COMPONENT_ORDER 换为实际 COMPONENTS，保留 environment.rs 序列化机制断言。
- 原 write-gate 28 个测试全部迁移，继续使用 node:assert，vitest 注册。#10 的 `/null byte/` 换 `/空字符/`；#11 `/tmp/a.pdf` 改拒绝，新增上传 UUID 正例，追加三种 doc_refs 路径拒绝与空数组正例，其余断言保留；#17 `/path/` 换 `/路径/`；#18 `/unknown open kind/` 换 `/未知的打开类型/`；#23 `/limit/` 换 `/上限/`。read #8 同样换 `/上限/`。
- 测试导入改为 `.ts`，`__dirname` 改 `import.meta.dirname`；冻结表的故意 push 加 `@ts-expect-error`，仍实际执行以验证冻结。这些是 TS/ESM 迁移必需变化。
- 上一轮审计结论是 M-01 通过，没有退回缺陷或必须逐字搬入的审计检查；红绿回归要求本轮不适用。

## 命令与结果

1. `npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && npm test --prefix apps/web && npm test --prefix apps/desktop`：lint/typecheck 通过；Web 10 文件、113 用例，106 通过、7 未执行，server.test.ts beforeAll 启动报 `listen EPERM: operation not permitted 127.0.0.1`，伴随 watcher `EMFILE`；Web 步骤退出 1，链式 desktop 未执行。原始输出 `.autome/output/m02-acceptance.txt`。
2. 单独 `npm test --prefix apps/desktop`：退出 0，104 用例，103 通过、0 失败、1 跳过（原有 electron dom harness）。输出 `.autome/output/m02-desktop.txt`。
3. `npm run lint --prefix apps/web && npm run typecheck --prefix apps/web && (cd apps/web && npx vitest run --exclude test/server.test.ts)`：退出 0，9 文件 106 用例全过、0 跳过。仅用于明确不依赖监听的覆盖范围，不替代全量验收。输出 `.autome/output/m02-runnable.txt`。
4. `(cd apps/web && npx vitest run test/read-gate.test.ts test/write-gate.test.ts)`：46 用例通过，输出 `.autome/output/m02-gates.txt`；负向对照恢复后的最终运行见 `.autome/output/m02-final-gates.txt`。

用例总数对比上一轮审计基线：Web 67 → 113（+46 = read 11 + write 35）。read = 原 8 + 新 3；write = 原 28 + 组件迁移 1 + 新 6。边界表通过循环在这些用例内部执行，没有删减旧测试或增加静默 skip。受阻的 7 个测试属于原 W1 server.test.ts，未从总数隐藏。desktop 104 → 104（差 0），原有跳过数保持 1。

## 标待审之前的自审清单

### 1. 情形表逐行

| 设计 §5 的 M-02 行/规则 | 实际检查与结果 |
|---|---|
| 读 13 项且是 READ_METHODS 子集 | 原读 #1–3：冻结、解析 dispatch.rs、逐项缺省参数放行；通过 |
| project.probe/events.since/env.detect/config.validate 内部读 | 新读测试逐项拒绝；通过 |
| 读字符串不能像路径 | `/`、反斜线、`~`、`..`、NUL 均拒绝；普通 id、中文、空串仍遵循原形状规则；通过 |
| 写表与 ROLES、COMPONENTS | 原写角色测试 + environment.rs 组件序列化核对；全部 op 逐项合法参数通过；通过 |
| 附件只接受上传编号 | 空数组、单 UUID、256 UUID 通过，路径/大写/少多一字符/换行/非数组/257 项拒绝；通过 |
| doc_refs 缺省或空数组 | 三种路径和普通 README.md 一律拒绝；缺省、[] 通过；通过 |
| 固定 onboarding 文件枚举例外 | AGENTS.md 和 docs/agent-project-profile.md 通过，其余路径拒绝；通过 |
| 添加项目与任意 path 方法不可达 | 解析 dispatch 分派，实际取 path 的 project.add、project.probe 两项在读写通道均拒绝；通过 |
| 日志只接收 id | session.log 拒路径；open.path 的 project/task/session/name 四字段各拒路径，log + session_id 正例通过 |
| §5 路由表 core.restart/六个本地 op/附件解析/其余转发 | 本轮只验证门禁放行及参数限制；路由执行、501、uploads.resolve 属 M-05，不在本轮实施 |

### 2. 失败分支

已实际执行：非对象请求、未知 method/op、非对象 params、不可 JSON 序列化循环、总长超限、嵌套对象/非法数组、数组项类型/长度/NUL、标识符/正文/文档超限、路径、未知打开 kind、theme/component/decision/role/runtime 枚举错误、附件错误、非空 doc_refs。原迁移测试及新增测试均通过；中文原因由读失败输入表和写消息源码逐项核对。

拒绝授权：方法/操作白名单拒绝已测；Host/Origin 由 M-04 覆盖。读盘失败、超时、外部服务不可达不适用：本轮生产代码是无 I/O 的纯校验函数。空输入已测：null/undefined/空数组请求拒绝，合法 op 的缺省 params 行为保留。

### 3. 全部迁移

无状态机，不适用状态 × 事件；两通道 × 方向已执行：读白名单逐项读通过，写方法读拒绝；写白名单逐项合法输入通过，READ_METHODS 的其它方法均不在写表。原 desktop 的 `env.detect` 是有意保留的显式写入口，同时是内核 READ_METHODS 成员；测试明确锁定这一例外，不擅自删表。角色保存/重置、技能绑定/解绑、三个主题、四组件 install/login 均覆盖。

### 4. 输入域边界

已执行：读 JSON 4096/4097；写 JSON 32768/32769；正文 0/1/8000/8001；文档 0/1/262144/262145；标识符 0/1/512/513；数组 0/1/256/257；数组项 1/1024/1025（空技能名拒绝）；UUID 空/精确长度/少多一位/大小写/尾换行，doc_refs 空/单元素/缺省。上限断言用固定字面量，不仅从实现导入值构造测试。日期、闰日、跨年不适用：这些门禁没有日期输入域。前导零 UUID 保留为字符串且通过。

## 负向对照

同一命令 `(cd apps/web && npx vitest run test/read-gate.test.ts test/write-gate.test.ts)`，每次只临时改一处并在 finally 恢复：

| 注入 | 结果（均退出 1） | 输出 |
|---|---|---|
| 写表加 project.add | 3 失败 / 43 通过 | `.autome/output/m02-negative-write-project-add.txt` |
| 关闭附件编号校验 | 2 失败 / 44 通过，含迁移 #11 | `.autome/output/m02-negative-attachment-path.txt` |
| 关闭 doc_refs 限制 | 2 失败 / 44 通过，含迁移 #11 | `.autome/output/m02-negative-doc-refs.txt` |
| 读门禁 import 写门禁 | 1 失败 / 45 通过 | `.autome/output/m02-negative-gate-import.txt` |
| 读表加 project.add | 2 失败 / 44 通过 | `.autome/output/m02-negative-read-project-add.txt` |

未增加任何长期禁用或 skip；以上临时改动均已恢复。
