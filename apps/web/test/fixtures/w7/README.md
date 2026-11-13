# W7 真实内核回包

这四份 JSON 是 `w7-flow.sh` 从独立临时仓库取得的原始回包，未手改字段。内含临时路径与时间戳，仅供纯函数测试使用；不可把里面的路径用于实时读盘。

抓取日期：2026-11-13。工作树 HEAD：`6dfc42a2628fb5c41e3ca7d4a30b0831b9877dbf`；使用当前 worktree 的 `target/debug/autome`、`target/debug/automed`（需求与 Bug 均已到 await_merge）。

二进制 SHA-256：

- autome：`d2b01f3c70257f8625d50ba5f8ad53f84f00c66c6bb4f31abeefa52a9727ebc2`
- automed：`6569a9f5e3b1702ef1c9627903af890719b743e7c4703563e595f974346f704a`

再生成（在仓库根执行；输出目录环境变量必须为绝对路径）：

```sh
cd apps/web
W7_CAPTURE_DIR="$PWD/test/fixtures/w7" npx vitest run test/task-detail-real.test.ts
```

夹具创建两个任务后，第一次 `autome run --until-idle` 同时推进到需求批准设计 / Bug 合并卡点；`autome approve feat` 后第二次 drain 推进需求到合并卡点。随后 main 前进一次提交。

原始取数命令（夹具自动设置临时 AUTOME_HOME）：

```sh
autome rpc task.get '{"task_id":"T-1"}'
autome rpc task.get '{"task_id":"T-2"}'
autome rpc task.changes '{"task_id":"T-1"}'
autome rpc events.since '{"after_seq":0,"limit":1000}'
```

真实集成用例始终重新创建任务，不用这些 JSON 冒充真实内核。`AUTOME_BIN` / `AUTOMED_BIN` 可指定其它版本。当前实现沙箱拒绝 `ps`，三个数据用例通过，第四个进程清理用例与 afterAll 报 EPERM；不能据此声称全部验收或无残留通过。
