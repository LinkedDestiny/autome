# M-08 审计 #10 独立复验

- 验收：driver_lock 11 通过；cli driver_ 4 通过；vitest 6 文件 100 通过；cargo build 通过。
- 负向对照（均已还原，工作区只剩文档改动）：
  1. `is_driving` 恒真 → `touch_updates_timestamp_and_driving_expires` 失败（10 过 1 败）。
  2. `ticking = true`（启动即登记）→ `driver_core_waits_for_first_tick` 失败。
  3. 去掉 `!driver.self` → web-launcher「使用登记」「只有本面板」两条失败。
  4. 删 read-gate 的 `driver.list` → 4 条失败（read-gate 3、write-gate 1）。
- 自造：`autome run` 真实运行时 `rpc driver.list` 含 `driving: true`；`kill -9` 后为空。
- 未取证：H-04（真实 launchd）在人工验收清单。
- 披露：清理用的 `pkill -9 -f automed` 过宽，可能误杀其他 automed 进程。
- 另：web-status-driver-real 在 vitest 内通过。
