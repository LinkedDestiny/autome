# M-04 审计复验（审计 #4）

结论：通过。

## 负向对照（先做，均已还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| `notification` 不再触发重新拉取 | bell-client 3 个用例失败（(1)(2)(6)） |
| `onMessage` 只对非内置类型调用 | bell-client 3 个用例失败（(4)(5)(6)） |
| `publishLiveMessage` 不捕获订阅者异常 | 订阅表用例失败 |
| 徽标阈值 `>= 100` 改 `> 100` | bell-view 1 个失败 |
| `TARGETS.available` 全改 true | bell-view 2 个失败（默认无链接、routes 守卫） |

## 复验
- `test/bell-client.test.ts test/bell-view.test.ts`：2 文件 13 用例全过（6+7，与契约一致）。
- Web 全量：32 文件 291 用例，0 跳过（与契约一致；监听型用例在本轮环境也能跑，实现轮的环境阻断不复现）。
- lint、typecheck、build 通过；desktop 104 用例 103 过 1 跳过（基线）。
- 宿主门禁 `ci-local.sh` 在 58c237a 通过（引用，不替代以上）。

## 未取证
视觉/真实浏览器项已在设计文档 `## 人工验收清单` H-01、H-05、H-07，不阻塞关闭。
