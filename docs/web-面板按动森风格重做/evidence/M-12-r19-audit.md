# M-12 审计复验（实现轮 r19）· 通过

- 验收命令：`cargo test -p automed --lib skills` 28 通过；vitest 四个文件 47 通过。
- 负向对照 1：dispatch.rs 把 `description` 回包改成 null，重编 automed 后 `skills-real.test.ts` 1 失败（真实描述用例）；还原并 touch 重编后 11 通过。（注意：真实夹具用 target/debug/automed，不重编会假绿。）
- 负向对照 2：删掉卡片描述行，`skills-page` 与 `skills-real` 共 2 失败；还原后通过。
- 判别检查：核对 parse_description 单测覆盖单行/多行折叠/块标量/空白/null/列表/损坏头部/BOM+CRLF/读盘失败回退；无需新增。
- 只读：扫描路径无写操作，真实用例断言来源与绑定不变。
- 所有注入已还原，git 工作区干净（除本轮文档）。
