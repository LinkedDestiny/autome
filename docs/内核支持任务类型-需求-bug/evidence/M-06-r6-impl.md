# M-06 · 实现 #6

## 范围与实现

`task.get` 顶层新增 `brief`，直接读取 `store.task_brief(task_id)?`；保留原始字节，无记录序列化为 null。未改 `task_json`、列表、看板或存储结构。上一轮审计通过，无退回检查需要迁入。

## 验证记录

- 新增 `dispatch::tests::task_card_read` 三个项目测试。修改前运行验收命令，0 passed / 3 failed（`.autome/output/m06-red.txt`）。最初归档用例的看板 ID 层级误写为 running[0].id，按现有响应改为 running[0].task.id，并补上任务对象无 brief 断言。
- `cargo test -p automed --lib dispatch::tests::task_card_read`：3 passed / 0 failed，总数 3，相比该过滤器上一轮基线 0 增加 3，分别覆盖原文、null、归档与列表隔离。日志 `.autome/output/m06-green.txt`。

## 标待审之前的自审清单

1. **情形表逐行**：有正文逐字节返回（中文、CRLF、换行、占位符、首尾空白）；无正文显式 null（检查字段存在）；草稿取消后状态 Cancelled 且已归档，仍返回原文；项目列表与非空 running 看板的任务对象均无 brief。三项测试通过。
2. **失败分支**：不适用；M-06 未点名新的授权、文件读取、超时或外部服务分支。读取沿用账本查询与 `?` 错误传播；无正文是成功的 null 分支，已测。
3. **全部迁移**：本接口只读，不新增状态迁移。涉及的 Draft → Cancelled + archived 已真实执行并断言；为确保看板检查不空跑，测试另以 Active 夹具检查非空 running 后恢复 Draft 再取消。
4. **域边界**：逐字节测试包含单字节正文、65536 字节正文（21845 个中文字加一个 ASCII 字节）、首尾空白与换行、占位符；无正文测试覆盖 None。空串在创建接口已被既有 M-02 测试拒绝，本轮不改变创建契约。

## 范围与环境说明

所有测试使用项目原有隔离账本，不访问真实账本。本轮只处理 M-06。当前 Git 基线仅跟踪脚手架，项目源码及任务文档起始即未跟踪；提交只指定本轮涉及文件，未将其余未跟踪文件或用户的 .gitignore 改动加入暂存。

## 最终门禁与负向对照

- `cargo test --workspace`：988 passed / 0 failed，总数 988，较上一轮 985 增加 3，全部来自 task_card_read（automed lib 577 → 580）；其它套件数量不变。日志 `.autome/output/m06-workspace.txt`。首次全量启动时仍含看板夹具错误，失败后修正再跑得到此结果。
- `cargo clippy --all-targets -- -D warnings` 与 `cargo fmt --all -- --check`：退出 0；日志 `.autome/output/m06-clippy.txt`、`m06-fmt-check.txt`。
- 将返回值临时改为 trim：验收 1 passed / 2 failed，原文和归档原文断言均抓到变化；日志 `.autome/output/m06-trim-negative.txt`。已还原，恢复后复验日志 `.autome/output/m06-restored.txt`。
