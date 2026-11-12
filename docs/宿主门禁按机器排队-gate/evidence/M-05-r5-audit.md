# M-05 复验证据（审计 #5，对应实现轮 r5）

## 验收命令
- `cargo test -p automed --test end_to_end`：第一次 111 过 / 1 败（`a_session_that_finished_during_a_restart_is_consumed_on_recovery`，等会话 `.exit` 标记的固定 10 秒超时，负载下 174% CPU 时超时；与门禁无关、单跑 3/3 通过、此用例属基线）。第二次全量 **112 过 / 0 败**，与契约一致（基线 106 + 6）。判定为既有的负载型抖动，非产品缺陷。
- `gate_queue_` 过滤：6 个全过，13 秒（≤60 秒）。`gate` 过滤列出 17（11+6），`setup_` 过滤列出 17（16 + T5 名字含 `setup_`），基线用例无缺失。
- `cargo test --workspace` 全绿：automed lib 684、domain 396、cli 32+14、end_to_end 112、prompt_rendering 16。

## 负向对照（均注入后跑、再还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| 准入恒放行（`admit` 总返回 true） | T1、T2、T3、T5、T6 失败（T4 本就不排队，通过正常） |
| 入队时写 `.started` 占位文件 | T1、T2、T3、T6 失败 |
| `held` 额外按固定全局路径（临时目录下所有 e2e 仓库）计数 | T4 失败 |
| domain `position` 比较反向（模拟顺序错乱） | T6 失败 |
| （无效尝试）仅反转 automed 侧 `sort_by_key`：T6 仍通过，因为名次由 domain 按 `seq` 现算，该排序冗余——不是测试缺口，已改注入 domain 并失败 | — |

## 结论
验收命令真能拦错误实现；产品行为符合设计 T1–T6 与验收 1–4。无实现缺陷、无验证缺口。
人工验收项 H-01 已在设计文档「人工验收清单」中，由用户合并前确认（非本里程碑会话可跑项）。
