# M-01 · 实现 #10 · 门禁依赖修复受阻

本轮为修复轮，不推进新里程碑。起始 HEAD 4aff9e7；用户报告失败提交 cdfd836d68。设计文档四个里程碑实际均为开放，保持原状态，implementation-round 更新为 10/20。没有审计文件，不存在需搬迁的审计检查；本次失败已由项目原有 typecheck 与测试导入路径覆盖。

## 复现、诊断与修复尝试

- 原样执行 sh scripts/ci-local.sh：退出 1，161 秒；日志 .autome/output/r10-ci-red.txt。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web lint、Web build 通过；Web typecheck、Web test 失败。
- npm run typecheck --prefix apps/web 单独复现 TS2307（找不到 ws）与派生 TS7006；日志 .autome/output/r10-typecheck-red.txt。该调用以 tail 收尾，外层退出 0 不代表类型检查通过；完整门禁记录该步骤失败。
- 排查假设依次为安装缺失、声明与锁文件不一致、模块解析问题。npm ls --prefix apps/web ws @types/ws 为空，package.json 与锁文件版本一致，支持安装缺失为直接原因。未改动源码、依赖版本或锁文件。
- npm cache add ws@8.22.2 --cache "$PWD/.autome/output/r10-npm-cache" --fetch-retries=0 --fetch-timeout=10000：退出 1，ENOTFOUND registry.npmjs.org；日志 .autome/output/r10-fetch.txt。
- npm cache ls 'ws' '@types/ws'：仅有旧版 ws tarball，未发现锁定版本；日志 .autome/output/r10-cache.txt。
- npm install --prefix apps/web --offline --ignore-scripts --no-audit --no-fund --logs-dir "$PWD/.autome/output/r10-npm-logs"：退出 1，ENOTCACHED ws-8.22.2.tgz；日志 .autome/output/r10-install-offline.txt。
- 当前权限禁止申请网络升级；安装没有成功，未取得绿色复验。不以替换版本、跳过测试或绕过模块导入消除失败。环境未改变，未重复同一门禁并声称通过。

## 用例总数与上一轮差值

Web 18 个文件，15 通过、3 导入失败；收集总数 164，164 通过、0 跳过。较第 9 轮基线 164 差 0，没有增删测试；较 M-01 预期 188 少 24：server/gateway-http 既有 19 个、eventbus-live 新增 5 个因 ws 缺失未收集。Desktop 总数 104，103 通过、1 基线跳过，较上一轮差 0。

## 标待审之前的自审清单（本轮不标待审）

1. **情形表逐行**：现有 eventbus 八项随门禁通过：初始化不推历史、2500 条三页、单飞合并补拉、失败保持游标并重试、重复页去重、hello 后严格递增、订阅退订、关闭丢弃在途结果。真实服务五项因导入失败未取证。M-02 的 resume 情形表不适用本轮依赖修复。
2. **失败分支**：读盘失败模拟与原游标重试、空页均由现有单测执行通过；拒绝外站 Origin、真实服务关闭未取证。下载 DNS 失败与离线缓存缺失实际执行。续传超时不适用，属 M-02。
3. **全部迁移**：现有单测执行未初始化→初始化、空闲→拉取→补拉→空闲、失败→重试、连接→断开、订阅→退订、运行→关闭及在途结果丢弃。真实升级与 SIGTERM 未取证；resuming/core 广播迁移不适用本轮，属 M-02。
4. **输入域边界**：现有单测覆盖游标 0、MAX_SAFE_INTEGER 初始化参数、空页、单事件、1000 条页、2500 条跨页、重复序号。无新增输入域；非法 resume 与 1000/1001 续传边界不适用，属 M-02。

## 结论

修复未完成：阻塞为锁定依赖无法下载且不在缓存中，不是协议失败。git diff 30e5d51 --stat -- crates apps/desktop protocol plugins 退出 0、输出为空；没有修改受保护目录。四个里程碑保持开放，不启动下一会话。最终门禁以 Autome 对本轮提交的重跑为准。
