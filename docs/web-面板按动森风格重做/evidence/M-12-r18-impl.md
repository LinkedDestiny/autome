# M-12 · 实现 #18

## 修改与范围

skills.list 按 inventory 已有稳定来源顺序读取 SKILL.md YAML 头部，返回首个非空字符串 description；缺失、读盘失败、非字符串或无效头部返回 null。复用 YAML 解析库处理引号、折叠及保留换行的多行文本，不自造 YAML 语法。只在展示回包读取，不改变技能清单、可见性或绑定规则。卡片用 React 普通文本显示描述，缺值显示「暂无描述」，保留全部来源路径；长文本自动换行。

## 验收与数量

- `cargo test -p automed --lib skills && (cd apps/web && npx vitest run test/skills-model.test.ts test/skills-page.test.ts test/skills-real.test.ts test/labels.test.ts)`：Rust 28、Vitest 47，共 **75** 用例全部通过，无跳过。恢复负向注入后再次同命令通过。日志 `.autome/output/r18-restored.log`。
- 对齐本里程碑范围的既有基线为 Rust 26 + Web 45 = 71，本轮 +4：Rust 头部域表与读取失败各 1，页面描述/转义 1，真实内核来源回退与页面一致性 1。M-09 已关闭同范围 Web 基线为 skills-model 16、skills-page 8、skills-real 10、labels 11 = 45，本轮为 16+9+11+11=47。与上一轮 M-11 的 25 个实际通过检查相比 +50，主要来自验收范围切换，不能解释为新增 50 个测试。
- 首次 Cargo 获取索引失败，缓存离线构建成功后固定 Cargo.lock；最终原命令正常执行。首次 Web 检查使用旧 automed 二进制，真实字段断言失败（46/47），`cargo build -p automed --offline` 后通过。日志 `.autome/output/r18-offline.log`、`r18-build.log`、`r18-final-accept.log`。
- 负向对照：临时删除卡片描述 span，skills-page 与 skills-real 两条描述断言均失败（18 过、2 失败），日志 `.autome/output/r18-negative.log`；已完整还原并重跑 75 例通过。旧二进制缺失回包字段也被真实集成断言抓住。
- 上一轮审计通过 M-11，没有 M-12 退回检查需要迁移。

## 标待审前四项自审

1. **情形表逐行**：单行中文、带冒号/井号的引号文本、折叠多行、保留换行多行由 Rust 域表通过；无头部/空描述/文件读取失败回退通过；真实双来源首项空→次项有效、首项有效优先、全部无效→null 通过，来源全保留；HTML 原样返回、React 转义显示通过。
2. **失败分支**：不可读取路径、非法 UTF-8、YAML 无效、缺失关闭标记、无描述键、空值均实测回退。授权拒绝与超时没有新增分支，已有 skills-real 的内核拒绝、配置失败、无网关检查本轮照常通过；外部服务不可达不适用，本读取不调用外部服务。
3. **全部迁移**：描述无值→次来源有效→首来源有效→全部无效在同一个真实内核中读回验证；没有新增写操作或状态机。原全局/项目绑定与取消、覆盖及部分保存拒绝用例全部通过，描述读取前后 visible_to、sources、bound_roles 保持一致，技能正文读回未变化。
4. **输入域边界**：空串、空白、null、数组、缺键、单字符、中文、BOM/CRLF、未闭合头部、HTML、多行文本已覆盖。日期/数值最大值不适用，该字段为只读文字，不新增日期或数字输入。

## 检查与限制

视觉沿用现有技能卡片；本轮未取得真实浏览器视觉证据，既有 H-07/H-10 人工检查仍保留，不将 SSR 证据当作浏览器验收。

新增 YAML 依赖锁定为本机缓存的 serde_yaml 0.9.34+deprecated（其版本明确带 deprecated 标记）；后续维护需要注意该依赖状态。本轮不自行实现不完整 YAML。离线解析曾调整无关 wasm 依赖，已恢复其原锁定版本，`cargo test --locked -p automed --lib skills` 28 例通过。

第一次指定快检与临时负向注入重叠，skills-real 失败被污染，因此不作为最终依据；已在完整恢复后的代码上重新执行相同命令。未执行 ci-local 全量门禁。

最终指定快检 `sh scripts/ci-quick.sh --since cb2918e1ed20ea21791ac56c9aa52724f7eb9a21` 退出 1：25 步、失败 3 步、111 秒，日志 `.autome/output/r18-quick-final.log`。Format、Clippy、Web lint/typecheck、构建及 skills 相关检查通过。失败项：load-context 6 条（调用次数期望 3 实际 4、离线返回形状、config 事件重验断言；本轮未修改 root 或该测试）；shell-live 1 条 `listen EPERM 127.0.0.1`；Web e2e 准备阶段 `spawnSync ps EPERM`。未将这三步记录为通过，需宿主处理/复验；不属于协议格式失败。`git diff --check` 通过。
