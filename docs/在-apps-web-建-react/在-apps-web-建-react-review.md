# 设计评审 #5

结论：**未发现属于协议「可以要求再评审一轮的六类问题」的阻塞问题。** 本轮不要求追加评审；是否定稿由裁决轮决定。非阻塞建议：无新增。

## 评审依据与范围

- 基线提交：`0bbf81f`。先读 `brief/review-5.md` 和任务文件，再独立复核完整设计；对照本任务固定的 `protocol/session-protocol.md`、`protocol/loop-protocol.md`、根 `AGENTS.md` 与 `.autome/rules/README.md`。
- 核对当前 `scripts/ci-local.sh`、`.github/workflows/ci.yml`、`apps/desktop/package.json`、已安装 Electron 包的 `index.js`，以及上一轮评审、裁决记录和 `retro.md`。只读检查，不修改上述文件。
- 任务 R-01..R-04 与范围约束仍为依据；Node 下限和 React Router 主版本按权威设计「用户决定（2026-11-10）」及设计 #2 运行记录所载变更解读：Node ≥22.22.0、React Router 8、服务端原生类型剥离。未把已被用户决定取代的旧下限作为新问题复提。
- 检查范围包含全部七节方案、四个里程碑及验收契约、风险安排、人工验收、Backlog 和争议项；合并同根因判断，不将测试增强建议升级为阻塞。

## 六类问题复核

| 类别 | 追溯依据 | 结论 |
|---|---|---|
| 关键事实错误 | R-01、R-02；设计 §2–§4 | 未发现。现有预验证包的实际依赖元数据与选型一致；Node 原生类型剥离前提及依赖导入通过本轮下述检查。 |
| 内部逻辑矛盾 | R-01..R-04；设计 §3–§7、M-01..M-04 | 未发现。先检查版本再动态导入 TS、dev/prod 分路、静态资源处理及单一监听服务器相容；各阶段用例计数为 12、24、30、36，与所列新增项相符。 |
| 违反项目规则 | 任务 §3、§4；AGENTS.md | 未发现。拟交付范围限于 apps/web 与两处 CI 文件；Electron 采用调用现有二进制的方式，禁止 require 触发下载，不安排修改 Desktop 或内核。 |
| 违反任务目标 | R-01..R-04；任务 §4；设计所载用户决定 | 未发现。双启动路径、中文版本拒绝、vitest 与四项 CI 检查、SQLite 双运行时探针均有落实位置；未加入 W2–W12 业务实现。 |
| 里程碑不可执行 | M-01..M-04 验收契约；协议硬判据 | 未发现。四个独立契约均具备「验收命令、预期、负向对照」三行；前置关系无环，R-01..R-04 均有覆盖。不以“验收还可更明确”要求复审。 |
| 必须提前验证的重大风险 | 设计「风险与验证安排」、§7、M-01/M-03/M-04 | 未发现无法在实现期及时验证且会使后续整体失效的未处置风险。启动兼容性优先放在 M-01，SQLite 探针在 M-03，首次安装及门禁耗时在 M-04；环境未取证不作为协议失败。 |

## 独立核对与证据边界

- 本轮读取 `.autome/output/plan2-probe/package-lock.json` 与实际安装的包元数据：React Router / dev / express 为 8.5.1，最低 Node 为 22.22.0；typescript-eslint 8.72.2 的 TypeScript peer 为 `>=4.8.4 <6.1.0`，与设计的 TS 6.0.3 相容。用该预验证包的 semver 逐项检查 lockfile 内所有 `engines.node`，不兼容 22.22.0 的项为 0。原始检查输出：`.autome/output/review-5-checks.log`。
- 本轮用 `/opt/homebrew/opt/node@22/bin/node`（实测 v22.23.1）直接动态导入已有预验证原型 `server/http.ts`，退出 0；导入包含原生 TS 及自定义服务器依赖，未调用监听函数。这只证明该原型可导入，不代表 dev/prod HTTP 或 HMR 验收通过。
- [Node 官方 TypeScript 文档](https://nodejs.org/api/typescript.html)确认 22.18.0 起默认类型剥离，并说明扩展名、`import type` 与不可剥离语法边界；设计 §3/§5 已安排对应约束。[React Router 官方框架安装页](https://reactrouter.com/start/framework/installation)显示当前文档版本 8.5.1，与本地预验证包相符。未将首次不可访问的 custom-server 页面作为证据。
- 当前 ci-local 的 `run` 函数会累计各步结果并最终非零退出；设计 §6 保持独立四行 Web 检查及缺 node_modules 才安装的结构。当前 workflow 的既有 Ubuntu 例外为 contract、cli-release；设计新增 Web job 仍明确使用 macos-14 和 timeout-minutes，未扩大改动范围。
- 已核对 Electron 包入口确会在缺二进制时尝试下载；设计 §7 禁止 require 并按 path.txt/dist 定位，与现状相符。本轮没有调用 Electron、修改 Desktop 安装或复跑 SQLite 双运行时探针。
- 状态块七个必需字段及四个验收契约的三行存在性检查通过。当前 apps/web 尚未实现；本轮没有安装依赖、执行里程碑验收、运行整套 ci-local、验证 Node 22.22.0 本体或进行浏览器人工验收。设计轮的预验证结果仍归属于设计轮，不改记为本轮实测。

## 既有意见与非阻塞建议

- D1-P01（对应 D4-P01）：锚定内容已被用户决定和设计 #2 改写，本轮按当前 Node ≥22.22.0 要求检查，不重复提出旧约束冲突。
- N4-01：当前 M-01 已要求同时删除两个静态处理中间件，原建议已落实。
- N4-02：当前 §3 已先 await 构建模块再传 build，原建议已落实。
- 非阻塞建议：无新增。不把设计已经明确保留到实现/人工验收的取证工作再次包装成设计问题。

## 协议与交付

本轮仅覆盖本评审记录，并按协议在 `retro.md` 追加一行；不修改设计文档、状态块、轮次、里程碑或代码。设计保持 `status: 设计中`、`design-round: 1/15`、`implementation-round: 0/0`、`current-milestone: M-01`、`current-milestone-reopens: 0`、`convergence-mode: normal` 与原有 next-action；不另造状态块。

结束前仅提交上述两份文档到当前分支，不推送，不启动下一会话。
