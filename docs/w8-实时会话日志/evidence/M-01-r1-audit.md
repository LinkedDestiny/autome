# M-01 r1 审计复验

## 验收命令
`npm test --prefix apps/web -- test/eventbus-protocol-log.test.ts test/logtail-chunk.test.ts test/eventbus-protocol.test.ts`：退出码 0，3 个文件、18 个用例通过（8+7+3，与契约预期一致），无跳过。
`npm run lint --prefix apps/web` 退出码 0；`npm run typecheck --prefix apps/web` 退出码 0。

## 负向对照（均已还原，git status 干净）
| 注入 | 结果 |
|---|---|
| 按原文字节计长、不计信封 | logtail-chunk 4 条变红（200KiB ASCII、中文、70KiB 转义、128 字符 id） |
| 切块不按字符边界（逐字节出块） | 4 条变红（含中文边界、残缺前缀、非法字节） |
| `parseLogSubscribe/Unsubscribe` 忽略未知键 | eventbus-protocol-log 2 条变红（额外字段、退订） |
注：第一次尝试 C3 时 sed 失败未注入（测试 8 绿），改用 python 重做后确认变红。

## 自造判别检查（通过，丢弃）
随机模糊 60 组（ASCII/引号/反斜杠/换行/ESC/中文/emoji/NUL，1/3 注入随机非法字节，session_id 36 与 128 字符，起点 0 与接近 MAX_SAFE_INTEGER）：每条编码后 ≤65536；offset/next 首尾相接；next 末值 = start+字节数；文本拼接 = `Buffer.toString('utf8')`；`final:false` 在随机点截断后用 `consumed` 续切，拼接仍等于整体解码。全部通过。

## 结论
通过。无人工验收项。
