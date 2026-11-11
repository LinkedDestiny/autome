# M-01 审计复验 · 审计 #1

结论：通过。

## 负向对照（先于验收命令，均已还原）
| 注入 | 结果 |
|---|---|
| 唯一索引改为 `(kind, task_id, event_seq)` | 「task_id 为 NULL 时同 kind 同 event_seq 去重」失败（1 failed / 13 passed） |
| 去掉「库比代码新」检查（`if (false)`） | 「库比代码新时抛中文错误且库不变」失败 |
| `commit` 拆成通知、游标两个事务 | 「commit 注入游标写失败时通知随之回滚」失败 |
还原后 `git status --porcelain` 为空。

## 自己跑的验收
- `npm test --prefix apps/web -- test/store.test.ts test/store-warnings.test.ts`：2 文件 17 用例全过。
- Web 全量：24 文件 242 用例（基线 225 + 17），0 跳过，退出码 0；lint 0；typecheck 0；desktop 104 个、103 过 1 跳过（同基线），退出码 0。
- 宿主门禁 `ci-local.sh` 通过（简报引用，未替代上述）。

## 新造检查
- Node 22.x（/opt/homebrew/opt/node@22）下装过滤后 import `node:sqlite`：「SQLite is an experimental feature」出现 0 次；随后发出的其它 ExperimentalWarning 仍打印 1 次。
- 阅读源码对照 §2：表结构、表达式唯一索引、迁移每级事务、WAL/busy_timeout、resetLedger、unreadByTask 均与设计一致。

## 未取证
H-06（Node 22 启动面板终端无告警）已由上面的子进程检查覆盖过滤逻辑；整进程启动留人工清单原有条目。监听类回归与本里程碑无关。
