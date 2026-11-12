# M-06 实现 #6 证据

范围：文档与全量回归。开工 HEAD 为 373815e；上一审计 M-05 通过，无退回缺陷，因此没有需要逐字搬迁、红绿复现的审计检查。本轮不新增用例、不改产品行为。

## 改动与修复

- README 配置节新增 concurrency 示例及全局缺省/范围、项目封顶、全账本计数、FIFO、预算/超时、读者与恢复说明；仅配置名额不会启用门禁命令。
- technical-design §8.1 同步宿主准备与门禁共享名额、接续资格、失败回收、未知占用、队列事件与字段；保留非内核 124 熔断摘要的兼容例外。
- 首次 clippy 发现 M-05 新增重启 e2e 的 Ctx::new 两处 needless_borrows_for_generic_args；首次 fmt 发现新增队列断言需换行。原始诊断在 m06-clippy.txt / m06-fmt.txt。仅删除两处 &、格式化一处 assert_eq，断言、输入、测试名称均未变；修后 clippy/fmt 退出 0（m06-clippy-green.txt / m06-fmt-green.txt）。这是本轮验收发现的问题，不是上一轮审计退回。

## 命令与结果

原始输出均位于 .autome/output/。

- cargo test --workspace -- --list：1262 个；对比 M-01 开工 m01-baseline.txt 的 1201 个，新增 61、减少 0（m06-list.txt / m06-list-delta.txt）。新增来自 M-01–M-05：domain 12、automed lib 36、CLI 7、e2e 6。本轮相对上一轮累计基线 1262 的差为 0。核对任务累计 crates diff，原有用例无删除、改名或修改；本轮仅调整本任务新增 e2e 的借用与格式。
- npm test --prefix apps/desktop：退出 0，共 105，104 通过、1 原有跳过、0 失败；相对基线差 0（m06-desktop.txt）。
- cargo clippy --all-targets -- -D warnings：修后退出 0，无诊断（Cargo 正常 Checking/Finished 输出不算诊断）。
- cargo fmt --all -- --check：修后退出 0，无输出。
- grep -n "concurrency" README.md 与 grep -n "concurrency" docs/development/technical-design.md：各退出 0，README 配置节及技术设计 §8.1 宿主准备/门禁均命中（m06-readme-grep.txt / m06-design-grep.txt）。
- git diff --check：通过。

## 标待审之前的自审清单

1. **情形表逐行**：M-06 无新增运行情形表。逐项核对其契约：README 配置说明已补；technical-design 宿主准备、宿主门禁两段已补；Rust 全量、clippy、fmt、桌面测试、旧名称清单逐项执行。全量覆盖 §5 的五种门禁与三种准备检出、准备到门禁接续、无命令/同 sha 复用/指纹未变/工作区跳过/verify 准备失败等路径，沿用项目测试，不复制临时审计程序。
2. **失败分支**：配置非法值由 gate_concurrency_*invalid_values；读盘未知占用由 gate_queue_unknown_directory_blocks_only_inflight_ledger_jobs；超时由 gate_timeout_real_poll_tail_matches_domain 与 gate_queue_time_does_not_count_toward_the_gate_timeout；队首出错由 gate_queue_unreachable_head_drops_queue_and_advances_followers；普通失败/非内核 124 由 gate_timeout_* 覆盖。全部纳入本轮全量。授权拒绝、外部服务不可达不适用本里程碑：文档无新增授权或远程服务接口。
3. **全部迁移**：全量中的 gate_queue_event_transitions 覆盖未排队/已排队 × queued、gate/setup/session.started、session.deferred、task.updated、queue_dropped、result/未知事件；调度用例覆盖暂停/停止/恢复重排、正常/失败/超时结束、Vanished、旧 head 清扫、准备接续及失去接续资格、未知目录阻塞后恢复、core 重启与升级超额收取。文档不新增状态或通道。
4. **域边界**：全量覆盖空队列/单元素、乱序和最大 seq、满额/超额、配置缺省与 1/10 边界、0/负数/11/小数/字符串/布尔/数组/空串/前导零拒绝、项目值全局封顶、null 与首名次显示 0 人。日期/闰日/跨年不适用，本次配置与队列接口无日期输入域。

## 限制

H-01 真实本机人工验收仍留给用户合并前确认，本轮不代称完成。宿主 scripts/ci-local.sh 由 Autome 调度，本轮不另行启动会话或宿主门禁。

修复后定向复验：cargo test -p automed --test end_to_end gate_queue_order_survives_a_core_restart，1 通过、111 过滤、退出 0（m06-restart-green.txt）。无用例数变化。

最终全量：cargo test --workspace 退出 0，总计 1262（1261 通过、1 原有忽略、0 失败）；其中 CLI 46、domain 397（含忽略 1）、automed lib 684、end_to_end 112、prompt_rendering 16、其它集成 7。总数与上一轮累计基线差 0，与原始基线 +61，来源见上；输出 m06-workspace.txt。全量运行期间所做代码改动仅为新增 e2e 的格式及两处借用，修后该重启用例已单独复验通过，所有目标 clippy 也已重新编译检查通过。自审四项执行完毕后标 M-06 待审，保留其余已审状态，implementation-round 为 6/30。
