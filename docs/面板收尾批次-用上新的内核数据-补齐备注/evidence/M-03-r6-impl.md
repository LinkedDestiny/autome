# M-03 · 实现 #6 · 快检修复，保持开放

## 范围与修复

本轮按用户末尾指令仅修复提交 0da1781e43 的两项快检失败，不推进新里程碑。当前设计表原本为 M-03 开放（上一轮浏览器验收受阻），保持开放、reopen 0；implementation-round 更新为 6/65。没有把任何里程碑标为已完成。

- 标签守卫：task-notes.tsx 的 reopen 文案直接包含“实现”“合并”，违反统一标签检查。改用 nodeLabel('implement') / nodeLabel('merging')，呈现文字保持不变。同类范围为该新增组件，完整 labels 守卫同时扫描 routes/components，修后通过。
- HTTP 合法请求夹具：WRITE_OPS 新增 note 后，遍历所有意图的请求缺少必填 text，action 正确拒绝、网关调用次数为零。补充 text='备注'，不改变生产端空输入校验，不改弱任何断言。检查其它意图参数，set_after 空数组合法，其余字段已有。既有 task-action 全意图精确参数用例通过。
- 上一审计结论是 M-02 通过，无待搬入的审计缺陷检查。本次宿主抓到问题的两条检查已经在项目测试体系，原样运行并保留；只修合法 HTTP 请求夹具。

## 命令与结果

1. 红：`npm run test --prefix apps/web -- test/labels.test.ts test/task-actions-http.test.ts`。日志 `.autome/output/m03-r6-red.txt`。总数 13：9 通过、1 失败、3 跳过。labels 第 7 项原样复现 task-notes.tsx 含“实现”；HTTP beforeAll 因 listen EPERM 127.0.0.1 失败，3 项未执行，未能在沙箱重现宿主的零次调用断言。
2. 修后：`npm run test --prefix apps/web -- test/labels.test.ts test/task-actions-http.test.ts test/task-notes.test.ts test/task-action.test.ts test/write-gate.test.ts test/test-groups.test.ts`。日志 `.autome/output/m03-r6-green.txt`。总数 86：83 通过、3 跳过；5 文件通过，HTTP 文件初始化仍被同一 EPERM 阻断。labels 已绿，HTTP 绿需宿主确认。与上轮里程碑单测基线 73 比为 +13（本轮额外选择 labels 10、HTTP 3），测试定义数量没有新增或删除；与本轮红运行 13 比为 +73（加入原里程碑的 4 文件）。不能与宿主 1022 个 related 用例直接比较，本轮选择范围不同。
3. 按用户最后一条指令运行原快检：`if [ -f scripts/ci-quick.sh ]; then sh scripts/ci-quick.sh; else cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && { [ -d apps/web/node_modules ] || (cd apps/web && npm ci); } && npm run lint --prefix apps/web && npm run typecheck --prefix apps/web; fi`。日志 `.autome/output/m03-r6-quick.txt`。23 秒，5 步通过：Format、Clippy、Web install、Web lint、Web typecheck。脚本明确提示“未提供本轮起点，未跑改动相关的测试”，因此不作为 related 或 HTTP 通过证明。最终快检以 Autome 提交后复跑为准。
4. `git diff --check` 通过。没有运行 ci-local；没有重新运行上一轮受阻的浏览器验收。本轮未起额外服务，HTTP 夹具在探测监听阶段即失败，未触碰真实账本。

## 标待审前自审清单

本轮不标待审；以下列明修复范围内实际检查及剩余缺口。

1. **情形表逐行**：task-notes 7 项全部通过，涵盖非终态表单、done/cancelled 说明、await_merge 独有 reopen、已/未送达、warning 与原文错误、离线禁用、旧数据与未知角色。labels 全视图扫描通过。真实网页简报送达与 CLI 五秒更新仍沿用上一轮未验缺口。
2. **失败分支**：task-notes / task-action / write-gate 中空白、超长、授权拒绝、超时、服务不可达与原文透传通过；HTTP 坏 Origin/Host 两项被初始化 EPERM 阻断，未宣称通过。读盘失败不适用：本轮未改文件读取。
3. **全部迁移**：本轮不改变状态机。既有测试逐个检查普通非终态、await_merge、merging/main_check/cleanup、done/cancelled 显隐与 reopen 参数；待送达/已送达渲染通过。真实送达迁移与 CLI 实时通道未补验，不能据此报待审。
4. **输入域边界**：现有测试检查空串、纯空白、1/8000/8001 字符与 reopen 有无，均通过；HTTP 夹具补充合法非空备注。前导零/闰日/跨年对本次标签与自由文本改动不适用，没有新增数字或日期解析。

## 下一步

由 Autome 重跑带起点的快检确认 HTTP 合法请求回归；后续会话复验 M-03 原验收命令及未完成浏览器负向对照，取得证据后才能标待审。
