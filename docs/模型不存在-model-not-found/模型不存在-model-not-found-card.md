# 模型不存在（model_not_found）被当成崩溃重试

## 现象
角色配置里的模型名在 CLI 那边不存在时（例如 Codex 返回 `model_not_found`），会话非零退出，内核把它当崩溃：退避重试两次后才失败，失败原因是「会话异常退出」，看不出是配置错了。`autome doctor`、`config.validate` 也不会提示。

## 复现步骤
1. 用假 CLI（`FAKE_CLI`）输出 Codex 的 `turn.failed`，错误信息含 `model_not_found` / `The model \`x\` does not exist`，并以非零退出
2. 让任务跑到这个会话
3. 看任务事件与最终状态

## 期望行为
识别为配置错误：不重试，任务直接进入 `Failed{Config}`，原因写明哪个角色、哪个 CLI、哪个模型不可用，以及去哪里改（`~/.autome/config.toml` 的 `roles.<角色>.model`）。

## 实际行为
`session.retry` 两次后 `Failed{SessionCrashed}`。2026-11-10 `gpt-6.1-sol` 下线时真实发生，T-8/T-9 的评审会话因此连续失败。

## 环境
- autome-v2 main @ fd519b2，macOS（Darwin 27），已安装的 `~/.local/bin/autome` 由同一提交构建
- 项目 autome-v2 自身由 tmux 里的 `autome run` 驱动

## 线索
`crates/automed/src/usage.rs` 的 `classify_failure` 只识别限流；可在同处加一类「配置错误」并由 `reap_session` 分流。Claude 侧对应的错误文本也要覆盖（各取一个真实样本做 fixture）。
