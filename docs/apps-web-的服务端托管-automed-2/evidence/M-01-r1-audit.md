# M-01 第 1 轮审计复验

未读实现轮证据，自行复验。

## 验收命令
`cargo build -p automed && lint && typecheck && npm test --prefix apps/web && npm test --prefix apps/desktop` 退出码 0。
Web：8 文件、67 用例（契约 67，差 0），skipped 0。desktop：104 用例，103 通过，1 跳过（electron 二进制缺失，设计「现状」已记录为基线）。宿主门禁（ci-local @ a92d0bb）也通过。

## 负向对照（逐项注入、跑、还原）
| 注入 | 结果 |
|---|---|
| framing `readUInt32BE`→`LE` | 11 个失败（framing 4 个 + sidecar 真实往返等） |
| 查找顺序 local-bin 与 cargo 对调 | core-binary「本地安装先于 cargo」失败，1 个 |
| sidecar 按到达顺序而非 request_id 关联 | 「乱序回复各归其主」失败，1 个 |
| ci.yml web job 删掉 `cargo build -p automed` | ci-config「测试前先构建内核」失败 |
| 该步移到 `npm test` 之后 | 同一用例失败 |
| 真实内核用例加 `test.skipIf(process.env.CI)` | 「测试源码不按条件跳过」守卫失败（CI=1 与否都失败） |

注：中途发现 `git checkout -- ..` 在 apps/web 下不会还原 `.github`，ci.yml 的注入一度残留；已发现并还原，最终 `git status` 干净，ci.yml 两项注入是在干净文件上各自重做的。

## 自造检查
- `AUTOMED_BIN=/nonexistent` 跑 sidecar 用例：9 个失败，含「binary is built」前置断言，缺内核一律红，不跳过。
- ci.yml 里该步确在 web job（`Typecheck` 之后、`Test` 之前）。

## 未取证项
无人工验收项。
