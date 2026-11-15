# 审计结论（审计 #10 · M-08）

结论：**通过**，M-08 标为已完成。

- 验收命令全段退出 0：`driver_lock` 11、cli `driver_` 过滤命中 4（3 个新用例 + 既有的 `write_command_waits_for_a_driver_lock…`）、vitest 6 个文件 100 个用例。
- 负向对照（逐项注入后还原，`git status` 干净）：`is_driving` 恒真 → driver_lock `touch_updates_timestamp_and_driving_expires` 失败；`serve_stdio` 启动即登记 → cli `driver_core_waits_for_first_tick` 失败；`status` 不排除本面板（去掉 `!driver.self`）→ web-launcher 两个用例失败；去掉 `read-gate.ts` 的 `driver.list` → read-gate 与 write-gate 共 4 个用例失败。
- 自造检查：临时 `AUTOME_HOME` 下 `autome run` 运行中 `rpc driver.list` 列出 `driving: true`，`kill -9` 后列表为空（陈旧登记按 flock 判定）。
- 真实 launchd 驱动的观感（H-04）已在人工验收清单，不影响关闭。
- 披露：自造检查中为清理用了 `pkill -9 -f automed`，范围过宽，可能误杀本机其他 automed 进程；检查结束时本机无 automed 进程，若宿主驱动曾在运行需 kickstart。
- 证据：evidence/M-08-r15-audit.md。
