# M-01 · 实现 #4 · 门禁修复轮

implementation-round：4/15。输入 HEAD：bc0be47；Autome 报告门禁提交：d566da508f。本轮不推进新里程碑，M-01 保持待审。不存在任务审计文件；失败检查已经在项目 test/app-protocol.test.js 中，无需复制或改写。

## 复现与修复尝试

- 原样运行 sh scripts/ci-local.sh，退出 1，用时 102 秒，日志 .autome/output/impl-4-ci-red.txt。Format、Clippy、Test、End to end、Adapter flags、Contract 通过，仅 Desktop 失败。两个 app-protocol 原有用例仍报 Electron failed to install correctly。
- 原样运行 node --test apps/desktop/test/app-protocol.test.js，2 个用例均失败；日志 .autome/output/impl-4-protocol-red.txt。保留现有用例与断言，没有跳过失败。
- 安装器读取的 Electron 包版本为 43.2.0。运行 electron_config_cache="$PWD/.autome/output/electron-cache" node apps/desktop/node_modules/electron/install.js，退出 1，报 TypeError: fetch failed；日志 .autome/output/impl-4-electron-install.txt。
- curl -I -L --max-time 15 https://github.com/electron/electron/releases/download/v43.2.0/electron-v43.2.0-darwin-arm64.zip 报 Could not resolve host: github.com；日志 .autome/output/impl-4-download.txt。检查 ~/Library/Caches/electron 与 ~/.cache/electron，没有发现匹配 43.2.0 的缓存包。
- 安装尝试失败后，全量门禁的 Desktop 阶段仍复现同样的两个失败。没有取得绿灯，没有伪造安装路径、替换版本或修改测试。任务 §4 禁止修改 apps/scripts；当前环境也不允许权限提升。安装环境阻塞不记为协议失败。
- 本轮没有产品代码修复；只更新状态、证据指针与本轮记录。上一轮 gate PID 与 driver_lock 失败本次没有重现，不宣称已修复。

## 验收结果与基线差

- Rust 全量 895 个（CLI 12、domain 288、automed lib 540、e2e 38、prompt 10、real_cli 7），全部通过；较上轮实际执行的独立 Rust 用例 879 增加 16，因为这次 lib 没有失败，prompt 10 和 real_cli 其余 6 得以执行。较计划中的现有完整基线 895 差 0。
- 门禁另行重复 e2e 38 与 Adapter flags 1，全部通过，不重复计入独立总数。
- Desktop 104 个，101 通过、2 失败、1 跳过，较上轮 104 差 0。门禁独立总数 999，较上轮 983 增加 16，原因如上；含重复运行共 1038 次用例执行。没有新增、删除或修改用例。
- M-01 专项 cargo test -p autome-domain requeued：9 个通过；cargo test -p automed --lib requeued：6 个通过；均较上轮差 0。日志 .autome/output/impl-4-domain.txt、.autome/output/impl-4-lib.txt。

## 标待审之前的四项自审清单

本轮保持既有待审状态，仍逐项执行自审。

1. **情形表逐行**：设计 §2 第 1 行 Paused→Requeued（13 节点）通过；第 2 行 Stopped 非门禁恢复通过；第 3 行 Stopped 三门禁带清理、Paused 不清理通过；第 4 行角色关闭时拒绝恢复通过；第 5 行新建 Queued→Intake 通过；第 6 行角色节点出队 Reenter 通过；第 7 行排队期间关闭角色→Failed 通过；第 8 行内核节点出队 RunCoreStep 通过；第 9 行五类控制事件拒绝通过；第 10 行 Cancel→Cancelled 通过。均由本轮 9 个域专项实际执行。设计 §1 四处排队处理由 6 个 lib 专项及全量测试覆盖，通过；R2 暂缓结论属于 M-03，本轮不适用。
2. **失败分支**：角色关闭拒绝/Failed、门禁目录被文件替代造成 I/O 失败、无 PID 等待超时、延迟 PID 清理、非法节点 JSON 均由专项实际执行并通过；外部下载不可达实测失败如上。授权拒绝不适用，本轮无新增授权接口。
3. **全部迁移**：域专项遍历 13 节点的恢复/出队、三个门禁的两种停法、角色开关、五类控制拒绝及取消；lib 专项检查槽满等待→腾槽回原节点、Stop 拒绝不杀原会话，全部通过。依赖安装自身无产品状态迁移；本轮未实现 M-02/M-03。
4. **输入域边界**：专项执行旧 queued 缺省/null、新状态往返、空串/未知字符串/数字节点拒绝、旧 paused/stopped、空/混合队列、单槽与门禁前缀隔离，全部通过。安装版本固定 43.2.0，未使用其它版本缓存。前导零、闰日、跨年、最大值不适用：本轮未新增日期或数字输入域。

## 结论

修复未完成：Electron 安装受网络解析阻塞，未取得红→绿证据。M-01 保持待审，不标已完成；implementation-round 为 4/15。H-01～H-03 未实测。最终门禁以 Autome 在提交后的提交上重跑为准。
