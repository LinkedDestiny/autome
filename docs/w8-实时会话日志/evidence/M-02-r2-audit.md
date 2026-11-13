# M-02 审计复验（实现 #2 之后，审计 #2）

未读实现轮证据，独立复验。

## 验收命令
- `npm test --prefix apps/web -- test/logtail.test.ts test/eventbus.test.ts`：2 文件 43 通过，skipped 0。契约写 39（17+22），实际 logtail 21 条（实现轮多 4 条：读取失败、重入、定时器节奏等），eventbus 仍 22。差额 4 全来自新增用例，无跳过。
- lint 退出码 0，typecheck 退出码 0。

## 负向对照（均已 `git checkout` 还原，工作区干净）
| 注入 | 变红用例 |
|---|---|
| 上限 `>=` 改 `>` | 04 |
| `close` 去掉 finally（只在正常路径关闭） | 08、13、18、19 |
| `close` 完全不关 | 19 条失败 |
| 跳过 `isKernelLogPath` 校验 | 03 |
| `final` 改为只看 `pos>=fstat` 且不看残缺尾部 | 17 |
| 先读、再查生命周期、结束就直接 `log.end`（契约原话） | 06、21 |
| `final` 恒 false / 其它 final 变体 | 06、07、14、17、20、21 |
验收契约点名的五种错误实现都能被拦住。

## 自造检查
上面的注入里 E/F/G 是额外构造的变体（正常路径关闭、读后查状态、final 忽略 consumed），均被至少一条用例拦住。结论：产品行为符合设计，无实现缺陷、无验证缺口。
