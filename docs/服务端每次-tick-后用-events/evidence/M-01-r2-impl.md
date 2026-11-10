# M-01 · 实现 #2 · 门禁修复轮，保持开放

本轮按用户最后指令只修复宿主门禁，不推进新里程碑。开始时 M-01 实际为「开放」，没有审计文件，故没有上一轮审计判别检查需要搬入测试。本轮保留全部里程碑状态，implementation-round 更新为 2/20。

## 根因、修复与环境阻塞

`eventbus-ws.ts` 和 `eventbus-live.test.ts` 已导入 ws，但 package.json 与锁文件未声明 ws 或其类型。补入设计指定的 `ws: ^8.22.2` 和 `@types/ws: ^8.18.2`。使用 npm 官方缓存元数据，经 `npm install --package-lock-only --offline --ignore-scripts --no-audit --no-fund` 分别生成依赖及开发依赖条目；没有手写 integrity，没有改版本，没有改变已有依赖的锁定条目。package.json 保持原有顺序，仅增加两项。

联网查询 `npm view ws@8.22.2 version engines --fetch-retries=0 --fetch-timeout=15000` 失败，registry.npmjs.org DNS 为 ENOTFOUND。默认缓存的 npm 离线查询也报 ENOTCACHED；用 cacache 只读 API 实查发现元数据存在，于是仅将这两项元数据复制到 worktree 的临时 npm 缓存，供 npm 正常生成锁文件。逐项比对锁文件的版本、resolved、integrity 与缓存元数据一致；既有 node_modules 锁条目全部与 HEAD 相同。

但按 integrity 查找两份实际 tarball 都为 ENOENT；随后 `npm install --prefix apps/web --offline --ignore-scripts --no-audit --no-fund --cache "$PWD/.autome/output/r2-npm-cache"` 退出 1（ENOTCACHED）。因此修复只完成声明和锁文件，运行环境仍缺包，不能宣称修复已验证或门禁通过。需要可访问 registry 的执行环境先安装依赖，再重跑同一门禁。未借用不同版本、未伪造类型声明、未删除或跳过失败测试。

原始日志均在 `.autome/output/`：`r2-registry.txt`、`r2-ws-metadata.txt`、`r2-types-metadata.txt`、`r2-lock-ws.txt`、`r2-lock-types.txt`、`r2-install.txt`；临时缓存为 `r2-npm-cache/`。

## 验收命令与用例基线

- 修复前 `sh scripts/ci-local.sh`：退出 1，123 秒；复现 Web typecheck 与 Web test 失败，其余九项通过。原始日志 `r2-ci-red.txt`。Web 18 文件中 15 通过、3 收集失败，164 用例通过。未安装依赖前，声明与锁文件不影响缺包复现。
- 修复后同一命令 `sh scripts/ci-local.sh`：退出 1，`r2-ci-after.txt`；仍是 Web typecheck 与 Web test 失败，其余九项通过，未取得绿色证据。
- `npm run lint --prefix apps/web`：退出 0，`r2-lint.txt`。
- `npm run typecheck --prefix apps/web`：实际 npm 退出 2，`r2-typecheck.txt`，ws 模块/类型缺失及连带隐式 any。
- `npm test --prefix apps/web`：退出 1，`r2-web.txt`，18 文件中 15 通过、3 收集失败，164 个用例全部通过，0 跳过；server、gateway-http、eventbus-live 仍无法加载。
- `npm run build --prefix apps/web`：退出 0，`r2-build.txt`；前端构建通过不证明自定义服务端可运行。
- `npm test --prefix apps/desktop`：退出 0，`r2-desktop.txt`；总数 104，103 通过、1 基线跳过。
- `git diff 30e5d51 --stat -- crates apps/desktop protocol plugins`：退出 0，`r2-scope.txt` 为空；`git diff --check` 通过。

Web 用例总数相对上一轮实际 164 的差为 **0**，未新增、删除或跳过测试。相对原始基线 175 的差仍为 -11（新增 eventbus 单元 8，原有 server/gateway-http 19 未收集）；相对 M-01 预期 188 仍缺 24（上述 19 加新增 live 5）。Desktop 相对上一轮 104 的差为 **0**，原有 1 跳过未变化。既有 Web typecheck/test 已能抓住此缺陷，未新增仅复述依赖声明的测试。

## 标待审之前的自审清单（本轮未标待审）

1. **情形表逐行**：本轮 Web 全量中的八项 eventbus 单元检查通过：初始化不推历史、2500 条三页、单飞合并补拉、失败保留游标、重复页去重、新连接 hello 与过滤、订阅退订、关闭及丢弃在途结果。五项真实服务检查均因缺包未收集，未取证。resume 情形表属于 M-02，本轮不适用。
2. **失败分支**：读盘失败的 Promise 拒绝与原游标重试、空页已由单元测试运行；Origin 拒绝仍未取证。registry 不可达和离线缺 tarball 均实测记录。内核超时、恢复续传属于 M-02，本轮不适用。
3. **全部迁移**：单元测试覆盖未初始化→初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭（含在途）。升级与 SIGTERM 的真实服务迁移未取证。M-02 的 core 广播和 resuming 迁移不适用本轮修复。
4. **输入域边界**：单元测试已运行游标 0、MAX_SAFE_INTEGER、空页、单事件、1000 条页、2500 条跨页和重复序号；本轮依赖版本及 integrity 已逐项核对。resume 的非法输入与 1000/1001 续传边界属于 M-02，不适用本轮。

## 结论与交接

没有绿色门禁证据，不标待审，不修改验收契约。M-01 继续开放，剩余真实服务验收与 onTick/升级守卫负向对照仍待依赖安装后执行。门禁最终结论由 Autome 在提交后的提交上复跑决定，本记录不替代它。未启动下一会话。
