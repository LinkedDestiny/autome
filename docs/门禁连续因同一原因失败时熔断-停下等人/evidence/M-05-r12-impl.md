# M-05 实现 #12：rebase 冲突修复与回归复验

## 范围与状态

本轮简报及审计 #9 均确认 M-01–M-05 已由审计关闭，没有开放或待审里程碑。本轮执行用户额外指定的 rebase 修复，不新增里程碑、不重新关闭里程碑、不改 reopen。状态块 implementation-round 为 12/25；H-01 仍待人工确认。上轮没有退回缺陷或新造检查，因此无须搬运审计回归用例。

## 修复

- `git rebase main`，目标 main 为 `2c32d12`；解决 config.rs、task.rs、config_io.rs、render.rs、end_to_end.rs 冲突。保留 main 的 Setup 配置、解析、失败原因、状态转换、CLI 测试，同时保留 GateSameFailure 的对应功能。
- 旧版 Bug Loop 测试采用 main 已有的 release-impl 文件屏障和 resume_wait，保留原节点、角色、会话数及最终状态断言。与 main 比较，end_to_end.rs 只有此前批准的两处 30s → 90s 期限替换及新增熔断测试。
- 首次批量消除冲突标记的脚本误截断文件尾部，fmt 和编译检查发现。用 ORIG_HEAD `81f654f`、共同祖先和 main 重新三方合并恢复五份完整文件，再逐块解决；最终净差异无此截断。原始输出在 `.autome/output/r12-*`。
- rebase 期间出现一次 index.lock 占用；重新执行 continue 后完成，未删除锁文件。`git merge-base --is-ancestor main HEAD` 成功，无未合并文件、未创建 merge 提交。

## 验收命令与结果

- 最终再次运行 `cargo test --workspace`：退出码 0，总数 **1189**，**1188 通过、0 失败、1 忽略**（T-14 真实日志测试），日志 `.autome/output/r12-workspace-recheck.txt`。其中 CLI 25、CLI 集成 11、domain 384、automed 644、e2e 101、prompt_rendering 16、real_cli 7 个通过。
- 与上一轮审计基线 **1142**（1141 通过、1 忽略）相比 **+47**：main 带入 CLI +2、domain +10、automed 库 +18、e2e +17；本轮冲突修复新增 0、删除 0。`r12-test-delta.txt` 按文件核对新增测试定义，总数一致。全部原熔断及恢复测试、Setup 测试、旧版 Bug Loop 兼容测试均在此次全量内通过。

- `cargo fmt --all -- --check`：通过，日志 `r12-fmt-final.txt`。
- `cargo clippy --all-targets -- -D warnings`：通过，日志 `r12-clippy-final.txt`。
- `cargo test -p autome-cli gate_same_failure`：2/2 通过，较原契约差 0，日志 `r12-cli.txt`。
- `npm test --prefix apps/desktop`：总数 105，104 通过、0 失败、1 跳过；较实现 #11 的 105 差 0。原有 Electron 缺失跳过仍在，日志 `r12-desktop.txt`。
- 首次最终源码上的 `cargo test --workspace`：automed 库 643 通过、1 失败（共 644），失败为 main 带入的 `gate::tests::requeued_discard_waits_for_the_wrapper_pid_before_removing_files`，断言旧 wrapper 已退出未满足，日志 `r12-workspace-final.txt`。未修改此用例或产品逻辑。
- 原样复跑 `cargo test -p automed --lib gate::tests::requeued_discard_waits_for_the_wrapper_pid_before_removing_files -- --exact`：1/1 通过，较首次选中用例数差 0，日志 `r12-gate-recheck.txt`。单次通过不能证明竞态不存在，保留首次失败记录供宿主门禁复验。
- `grep -c same_failure_limit README.md` = 2；`grep -c 熔断 docs/development/technical-design.md` = 1。
- `git diff --numstat main -- crates/automed/tests/end_to_end.rs apps/desktop/test`：end_to_end 为 164/2，labels-failure 为 17/0。五份冲突文件相对 main 删除内容仅上述两处期限及 task.rs 的 ExtendBudget 匹配行（增加 at 绑定），没有删除旧测试行为断言。
- `git diff --check`：通过。宿主 `sh scripts/ci-local.sh` 留给 Autome 提交后执行，本轮不声称完整宿主门禁通过。

## 四项自审

1. **情形表逐行**：本轮无新增情形。保留设计 §2 六种计数情形（无历史、通过、旧事件、已熔断、不同指纹、同指纹）及 §3 各节点适用范围，由项目 gate_failure、same_failure 测试复验；Setup 与 gate_same_failure 配置、状态机、读者分支均独立保留。CLI 文案和恢复提示 2 个、桌面读者 1 个均通过。
2. **失败分支**：拒绝非适用节点/不匹配节点、非法配置、零追加预算、空日志兜底、读盘失败（缺失文件与目录路径）、超时指纹均保留项目原测试，纳入全量复验。拒绝系统授权、外部服务不可达不适用：此次冲突修复不新增授权或外部服务调用。进程清理首次失败与复跑结果如上，未隐藏失败。
3. **全部迁移**：保留 Gate/MergeGate/FixGate 熔断、同门禁 rerun、角色 rerun、extend、拒绝其它门禁 rerun、非计数节点拒绝等原测试；main 的 SetupFailed 迁移亦保留。旧版 Bug 批准→Impl 观察→释放→await_merge→done 的原测试保留 main 屏障实现，纳入全量复验。
4. **域边界**：保留配置缺省 3、合法 1/10、非法 0/11/负数/小数/字符串；指纹空串、400 行窗口、256 KiB 日志读取边界、160 字符摘要及 TS 标识符数字测试。日期、闰日、跨年不适用：本次功能无日期输入域。本轮未新增或删减测试。
