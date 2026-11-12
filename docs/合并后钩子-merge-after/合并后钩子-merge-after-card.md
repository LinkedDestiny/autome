# 合并后钩子：`[merge] after`，由宿主在主检出上执行
## 背景
Autome 用自己的 Loop 开发自己：每次含 `crates/` 的任务合并后，都要有人手动 `sh scripts/install-cli.sh` 并重启驱动，否则驱动继续跑旧二进制（11-11 漏装一次导致 T-16 被旧 bug 判协议失败）。一般用户也有同样需求：合并后自动部署、重建索引、发通知。现在配置里没有任何钩子（`config.rs`、`config_io.rs` 无 hook/after_merge）。
## 要做什么
1. 项目 `.autome/config.toml` 新增：
   ```toml
   [merge]
   after = "sh scripts/install-cli.sh"   # 可省；用 sh -c 跑
   after_timeout_minutes = 10            # 可省，缺省 10
   ```
   解析规则与 `[gate]`/`[setup]` 一致（非表报错、空命令等于没配、超时须为正整数）。
2. 时机：任务合并成功且（配了 `[gate]` 时）验证 main 通过之后、清理之前，在**主检出**（`project.path`）上以独立进程组执行，复用 `gate::Job` 后台作业机制（`.autome/output/gates/<task>/after_merge-<sha>.*`），tick 轮询不阻塞。环境变量给 `AUTOME_TASK`、`AUTOME_MERGE_COMMIT`。
3. 结果：写事件 `merge.after {ok, exit_code, log, tail}`；失败只 `raise_alert("after_merge_failed")`（任务已合并，不能撤回），任务照常 Done；`autome run` 打印开始/结果两行。
4. 工作区项目：按成员仓库各跑一次或跳过并写 `merge.after_skipped`，设计轮定。
5. 本仓库的 `.autome/config.toml` 不在本任务里改（由用户合并后自行配置）。
## 不做什么
不做合并前钩子；不改 `[gate]`/`[setup]` 语义。
## 验收标准
e2e：配了 `[merge] after` 的任务合并后，主检出上出现钩子产物且事件序号在 `main_check` 之后、`cleanup` 之前；钩子失败 → 一条 `after_merge_failed` 警报、任务 Done；未配时零差异（无事件）。`cargo test --workspace` 全绿；README `[gate]` 旁补 `[merge]` 说明。
## 参考
T-27 的 `crates/automed/src/setup.rs`（同一套作业机制）、`crates/automed/src/gate.rs`、`scheduler.rs` 的 `run_core_step(Cleanup)`。
