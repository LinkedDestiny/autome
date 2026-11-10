# M-01 · 实现 #3 · 门禁修复轮

implementation-round：3/15。输入提交：66c516c（Autome 报告的门禁提交 b57676e）。M-01 保持待审，不推进新的里程碑。没有审计文件，也没有需要迁入测试体系的审计临时检查。

## 复现与修复尝试

- 原样执行 `sh scripts/ci-local.sh`，退出 1，用时 82 秒；日志 `.autome/output/r3-ci-red.txt`。Desktop 原有两个 app-protocol 用例再次报 `Electron failed to install correctly`。
- `node --test apps/desktop/test/app-protocol.test.js` 原样复现，2 个均失败；日志 `.autome/output/r3-desktop-red.txt`。已有用例保留，不复制、不删改、不跳过。
- 检查安装缺失、版本不符、路径异常三个假设：Electron 包锁定 43.2.0，缺 dist/path.txt；现有缓存只有 44.0.0、44.7.0，不能替代所需版本。
- 执行 `electron_config_cache="$PWD/.autome/output/electron-cache" node apps/desktop/node_modules/electron/install.js`，安装器报 `TypeError: fetch failed`，未完成安装；日志 `.autome/output/r3-electron-install.txt`。
- 对官方 43.2.0 darwin-arm64 发布包执行 `curl -I -L --max-time 15`，报 `Could not resolve host: github.com`；日志 `.autome/output/r3-download.txt`。当前环境禁止权限提升，没有匹配的离线缓存，无法完成下载。
- 没有改产品代码、依赖清单或门禁，没有伪造安装路径。任务 §4 排除 apps/scripts 改动；本轮已定位的 Desktop 阻塞仍是依赖安装环境。

首次门禁还出现 Rust 既有用例 `gate::tests::a_passing_command_is_done_with_zero_and_its_output_is_kept` 失败：`the wrapper removes its pid on the way out`。源码 WRAPPER 先 mv 发布 exit，再 rm PID；poll 可在两条命令之间看到 Done，所以这次失败有明确的时序解释。单独原样重跑通过（1 个用例，日志 `.autome/output/r3-gate-targeted.txt`），不把重跑通过表述为修复。此次未修改该路径，此项作为另一个门禁风险保留。

## 验收与基线差

- M-01 专项：`cargo test -p autome-domain requeued` 9 个通过；`cargo test -p automed --lib requeued` 6 个通过；相较实现 #2 都是差 0。日志 `.autome/output/r3-domain.txt`、`.autome/output/r3-lib.txt`。
- 首次门禁 Rust Test 执行 840 个（CLI 12、domain 288、lib 540），839 通过、1 失败；因 lib 失败，后续集成测试未在该步骤运行。另行 End to end 执行 38、Adapter flags 执行 1，均通过。Rust 独立用例实际执行 879，相比上轮 895 少 16：prompt 10、real_cli 其余 6 未执行，不能报作全量通过。
- Desktop 总数 104，101 通过、2 失败、1 跳过，相较上一轮 104 差 0。首次门禁合计独立用例 983，相较上一轮 999 少 16，原因如上；未新增、删除或修改用例。

## 四项自审清单

1. **情形表逐行**：本轮 15 个专项用例复验了 Paused/Stopped 恢复、门禁清理区分、13 节点出队、角色关闭拒绝/Failed、控制事件拒绝/取消、排队计数/顺序/槽位、腾槽回原节点、Stop 无副作用拒绝、手动复盘拒绝、旧账本往返，全部通过。本轮没有新增情形表；R2 暂缓结论仍属 M-03，不适用。
2. **失败分支**：专项覆盖角色关闭、清理 I/O 失败、无 PID 超时、延迟 PID、非法节点 JSON；全部通过。Desktop 外部下载不可达实测失败，已记录。授权拒绝不适用，本轮没有新增授权接口。
3. **全部迁移**：专项覆盖 13 节点的 Paused/Stopped→Requeued、Requeued 出队映射、关闭角色拒绝/Failed、控制事件拒绝与取消，以及槽满等待/腾槽重入；全部通过。本轮依赖安装不引入状态迁移，不推进 M-02/M-03。
4. **输入域边界**：专项覆盖旧 queued 缺省/null、合法与非法 resume、旧 paused/stopped、空/混合队列、单槽与门禁文件前缀隔离；通过。Electron 核对锁定版本与平台，未用不匹配缓存替代。日期/数值最大值不适用，本轮没有新增对应输入域。

## 结论

安装修复未完成，未取得红→绿证据。M-01 保持待审；环境阻塞不是协议失败。H-01～H-03 未进行人工验收。最终门禁由 Autome 在提交后重跑决定。

最终同命令复验：`sh scripts/ci-local.sh` 退出 1，用时 75 秒；日志 `.autome/output/r3-ci-after.txt`。Format/Clippy/End to end/Adapter flags/Contract 通过，Test/Desktop 失败。Desktop 仍为 104 个、101 通过/2 失败/1 跳过，差 0。Rust Test 仍执行 840 个（839 通过、1 失败），但失败变为 `driver_lock::tests::a_second_holder_is_refused_until_the_first_lets_go`，在 `driver_lock.rs:112` 断言释放后可重新获取失败；本轮未定位或修复这项失败。第一次的 PID 用例本次通过。额外 e2e 38 与 adapter 1 通过，独立总数仍为 983，与首次差 0、与上一轮差 -16（原因见前述）。两次失败不同，不能把任何一次描述成 Rust 全量通过。

`git diff --check` 通过。只更新本轮证据、状态/指针和一行 retro，不改里程碑状态。本轮没有完成门禁修复；后续除补齐 Electron 外，还需复验并定位以上 Rust 门禁不稳定项。
