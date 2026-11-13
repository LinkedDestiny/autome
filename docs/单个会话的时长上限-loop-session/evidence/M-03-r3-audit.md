# M-03 · 实现 #3 · 审计 #3 独立复验

结论：通过。

## 验收命令
契约命令原样运行，退出码 0。用例数：autome-domain 2、automed lib 6、prompt_rendering 3、e2e 2、回归子集 5，共 18，与契约一致；`end_to_end.rs` 删改行数为 0。原始输出：`.autome/output/a3-base.log`。

## 负向对照（逐项注入、跑测试、`git checkout` 还原；还原后工作区无被跟踪文件改动）
脚本与日志：`.autome/output/a3-mut.sh`、`a3-mut.log`、`a3-mut2.sh`。

| 注入 | 结果 |
|---|---|
| 超时仍走限流判定（`transient` 不因超时置空） | ③ 失败 |
| `next_round` 不排除 timed_out | ⑥ 失败 |
| `Inject::timed_out` 不展开内层 | ① 失败 |
| `effective()` 恒返回自身 | ② 失败（lib 侧栈溢出中止） |
| 渲染不替换「照常加一」 | ⑤、⑩ 失败 |
| 重派注入丢掉 `carried` | ⑦ 失败（⑩ 金样直接构造注入，不受影响，由 ⑦ 拦住） |
| 超时判据不读 `session.timeout` 事件 | ⑫、⑬ 失败 |
| 修复守卫读顶层变体 | ⑧ 失败 |
| Intake 不渲染注入 | ⑪ 失败 |

契约列出的每条负向对照都能让验收失败。

## 结论
无实现缺陷、无验证缺口。人工验收项 H-01、H-02 本就在清单中，未改。
