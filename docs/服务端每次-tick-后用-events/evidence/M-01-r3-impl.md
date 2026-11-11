# M-01 · 实现 #3 · 门禁修复受阻

本轮按最后指令只处理门禁失败，不推进新里程碑。开始时 M-01 实际为「开放」，全部里程碑状态保持不变；implementation-round 更新为 3/20。没有审计文件，没有需逐字迁入的审计判别检查。现有项目 typecheck 与 Web test 已原样复现缺陷。

## 复现、修复尝试与结果

- `sh scripts/ci-local.sh`：退出 1，126 秒。Web typecheck、Web test 失败，其余九项通过。原始输出 `.autome/output/r3-ci-red.txt`。TS2307 指向 eventbus-ws.ts 与 eventbus-live.test.ts 的 ws 导入；另外四个 TS7006 为缺少类型声明的连带错误。三个测试文件在收集阶段因找不到 ws 失败。
- 上轮已提交 package.json 与锁文件中的 ws 8.22.2、@types/ws 8.18.2。本轮确认根因仍是依赖未安装，不是遗漏声明；未修改业务源码或降级锁定版本。
- `npm view ws@8.22.2 version --fetch-retries=0 --fetch-timeout=10000`：失败，ENOTFOUND registry.npmjs.org，日志 `.autome/output/r3-registry.txt`。
- `npm install --prefix apps/web --ignore-scripts --no-audit --no-fund --fetch-retries=0 --fetch-timeout=10000 --cache "$PWD/.autome/output/r3-npm-cache"`：退出 1，ENOTFOUND，日志 `.autome/output/r3-install-online.txt`。
- `npm install --prefix apps/web --offline --ignore-scripts --no-audit --no-fund --cache "$PWD/.autome/output/r2-npm-cache"`：退出 1，明确为 ws-8.22.2.tgz 未缓存（ENOTCACHED），日志 `.autome/output/r3-install.txt`；使用本轮独立缓存也退出 1，同一原因，日志 `r3-install-offline.txt`。
- `npm cache ls` 检查发现默认缓存列出的 ws tarball 为 8.21.0、8.22.0、8.22.1，没有锁定的 8.22.2。日志 `r3-all-cache.txt`。未用旧包冒充锁定版本，未手写 ws 类型或绕过失败测试。
- 安装尝试期间门禁继续运行，最终仍为上述两项失败。由于安装未成功且源码未变，没有可验证的修复，不将同一失败描述成绿色复验。
- `git diff --check` 退出 0；`git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` 退出 0、输出为空。

## 用例总数与基线差

Web：18 文件，15 通过、3 收集失败；实际收集总数 **164**，全部通过、0 跳过。与上一轮实际总数 164 的差为 **0**，未增删用例。相对 M-01 预期 188 缺 24：server/gateway-http 的既有 19 项与 eventbus-live 的新增 5 项因缺 ws 未收集；相对任务原始基线 175 为 -11（新增 eventbus 8，缺少原有 19）。不能把 164 全过当成全量通过。

Desktop：总数 **104**，103 通过、1 基线跳过、0 失败；相对上一轮总数差 **0**。本轮未改 Desktop。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：本轮门禁实际执行 eventbus 的八项测试：初始化不推历史、2500 条三页、单飞合并补拉、失败保留游标、重复页去重、新连接 hello 与过滤、订阅退订、关闭并丢弃在途结果，全部通过。M-01 的五项真实服务检查缺 ws 未收集，均未取证；M-02 resume 情形表不适用本轮依赖修复。
2. **失败分支**：模拟读盘失败、保留游标后重试与空页已通过既有单元测试；外站 Origin 拒绝未取证。在线 DNS 失败与离线缺包实际运行确认。内核超时与恢复续传属 M-02，本轮不适用。
3. **全部迁移**：既有单元测试覆盖未初始化→初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭（含在途）。真实升级与 SIGTERM 关闭因缺包未取证。M-02 的 resuming/core 广播迁移不适用本轮。
4. **输入域边界**：已执行游标 0、MAX_SAFE_INTEGER、空页、单事件、1000 条分页、2500 条跨页与重复序号检查。resume 非法输入、1000/1001 续传边界属 M-02，本轮不适用；依赖安装使用现有锁文件未改变版本边界。

## 结论

门禁失败已复现，但当前环境无法取得锁定依赖，修复未完成。M-01 保持开放，不改验收契约，不声明待审或已完成。下一执行环境需先安装锁文件依赖，再原样运行 `sh scripts/ci-local.sh`；门禁通过后才继续 M-01 剩余验收和负向对照。本轮只更新证据、状态与一行运行记录。最终门禁由 Autome 在提交后重跑；本会话不启动下一轮。
