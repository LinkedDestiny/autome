# Grader：轮末启动即结束（不等待）

## 判定目标
实现轮（AT3）本轮末用启动脚本拉起 AT4 会话（脚本输出 `LAUNCHED:`）后**立即结束**，
不 `wait`、不轮询新会话日志、不重复启动。

## 机检优先
从 transcript 判定：
- 存在一次经 `new_claude_session.sh`（或 SESSION_SKILL 所述方法）拉起、脚本回显 `LAUNCHED:`。
- `LAUNCHED:` 之后 agent 无进一步操作即结束（无 `tail -f`/反复读取 `.autome/output/sessions/*.log`/再次调用启动脚本）。
- 满足 → 1；出现等待/轮询/重复启动 → 0。

## 阳性对照（C-22，必做）
> 应判负样本：transcript 在 `LAUNCHED:` 后出现 `tail -f .autome/output/sessions/...` 或第二次启动脚本调用。
> 期望 grader 给 0。若给 1，grader 失效，先修 grader。
