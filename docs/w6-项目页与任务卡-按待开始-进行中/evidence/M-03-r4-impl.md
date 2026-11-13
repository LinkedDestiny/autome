# M-03 · 实现 #4

结论：上传暂存、HTTP 端点、客户端及网关/上下文接线完成；按设计 §6 的受限环境契约报待审。31 个非监听用例通过；6 个 HTTP 用例均因 `listen EPERM: operation not permitted 127.0.0.1` 失败，尚不能声称 HTTP 或真实上传后创建链路通过。审计需在允许监听的环境复验。

## 改动与范围

- `uploads-limits.ts` 集中单文件字节上限、附件个数上限及中文超限文案；`uploads.ts` 清洗名字、按 UTF-8 字节截短主名保留扩展名，流式计数，先写 `.partial-UUID` 再重命名发布。
- `resolve` 只接受小写 UUID 和恰好一个普通文件的真实目录；不解析半成品或符号链接；`describe` 读取实际文件大小；暂存文件不自动删除。
- `/api/uploads` 放在安全中间件之后、JSON 解析之前，保留 JSON 写 API；拒绝错误媒体类型，失败返回中文。流式超限退出迭代时不销毁请求，以便返回 413；HTTP 层恢复消费剩余请求体。
- `main.ts` 将同一个 uploads 实例交给真实网关和 HTTP；开发/生产 SSR 上下文均增加可选 uploads。客户端 `uploadFile` 预检与服务端同用常量。
- 新增共享真实内核夹具 `w6-project.sh`，供后续里程碑复用；本轮不推进 M-04/M-05。
- 上轮审计为 M-02 通过，未退回实现缺陷，因此无须搬入缺陷检查或提供该类红绿记录。

## 命令、数量与结果

1. 修改前：`cd apps/web && npx vitest run test/gateway.test.ts test/load-context.test.ts`：2 文件、22 用例通过（gateway 13、load-context 9），0 失败、0 跳过。原始输出 `.autome/output/m03-baseline.txt`。
2. 最终验收：`cd apps/web && npx vitest run test/uploads.test.ts test/uploads-http.test.ts test/gateway.test.ts test/load-context.test.ts`：4 文件、**37 用例，31 通过、6 失败、0 跳过**；6 个失败均是监听 EPERM，非断言失败。原始输出 `.autome/output/m03-final.txt`，耗时 13.38 秒。受限的真实 fixture 设置/清理 2355 ms，未进入 HTTP 业务，不能据此计算完整链路耗时；该用例超时 120 秒，子进程设置超时 30 秒。
3. 可比基线差 **+15**：uploads 8、uploads-http 6、load-context +1；gateway 13 不变。上一实现轮 M-02 验收为 27，本轮 37 表面差 +10，原因是验收文件集合不同，不能视为累计项目新增数。本轮没有删除、跳过现有用例。
4. 两次缺陷注入均执行 `cd apps/web && npx vitest run test/uploads.test.ts`（各 8 用例）：去掉流式上限判断 → 第 3 个失败，7 通过；绕过文件名清洗 → 第 4 个失败，7 通过。输出 `.autome/output/m03-negative-stream-limit.txt`、`m03-negative-filename.txt`。两次均恢复源码，最终验收 uploads 8/8 绿。
5. `node --input-type=module -e "import('./apps/web/server/uploads.ts').then(() => console.log('Node direct import OK'))"` 通过，确认无 Node strip-types 不支持的参数属性语法。`git diff --check` 通过。
6. 未自行运行全量门禁、宿主快检、全量测试；验收中的原有 load-context 用例会调用自身必需的生产构建。Host/Origin 路由顺序与 main 接线的 HTTP 负向对照因监听限制未执行，留待审计。

## 标待审之前的自审清单

### 1. 情形表逐行

设计 §3 上传落法各行对应：

- URL 解码、斜杠/反斜杠取末段、去控制字符和前导点、空名拒绝、长主名保留扩展名：uploads (4) 通过；异常百分号 400，中文多字节截短和恰好 200 字节均覆盖。扩展名自身达到 200 字节、无法保留时返回中文 400。
- 声明长度立即拒绝、流式超限停止并清理：uploads (3) 通过；前者尚未创建 root，后者 root 为空。
- 半成品不可见、完成后 UUID 发布且字节一致：uploads (1)/(6) 通过。
- 小写 UUID、唯一普通文件、非法/不存在/空目录/双文件/子目录拒绝：uploads (5) 通过。
- describe 名字大小、真实网关翻译路径、保留源文件：uploads (2)/(7) 通过。
- 共用上限、客户端提前拒绝：uploads (3)/(8) 通过。
- HTTP 媒体类型、Host/Origin、中文 413、JSON 共存、真实 startServer→上传→建草稿：测试已经加入，uploads-http (1)–(6) 全部在监听时受阻，未取到行为证据。
- 可选上下文服务及旧调用：load-context 新增用例通过；两处生产代码注入均已核对。

### 2. 失败分支

- 授权拒绝：uploads-http (2) 含错误 Host 与错误 Origin，EPERM 阻塞未实证；源码位置确认位于安全中间件之后。
- 输入流读失败：uploads (6) 通过，已写入后抛错，半成品删除且无可解析 UUID；读目录失败：uploads (5)/(7) 返回 null。
- 磁盘写失败：以文件作为 home 的父路径触发 ENOTDIR，uploads (6) 返回中文 400。
- 空输入：空文件允许且 size=0；空文件名、`.`、`..` 拒绝 400，uploads (1)/(4) 通过。
- 超限：声明/流式两条拒绝及清理均通过 uploads (3)；HTTP 413 尚待监听环境复验。
- 超时、外部服务不可达：上传服务自身无外部依赖或设计指定超时状态，上传部分不适用；本轮 gateway 原有超时 504、离线 503 用例仍通过。客户端网络错误返回中文，但未另立独立网络故障用例。

### 3. 全部迁移

- 未开始→输入校验拒绝（无目录）：uploads (3)/(4)。
- 未开始→partial（仍不可 resolve）→完成 UUID（唯一文件、可 resolve/describe）：uploads (1)/(6)。
- partial→流式超限→目录删除：uploads (3)。
- partial→流失败→目录删除：uploads (6)。
- 未开始→磁盘失败→无发布文件：uploads (6)。
- 已发布→gateway 解析传递后仍保留文件：uploads (2)。真实 HTTP 建草稿后仍保留的迁移写入 uploads-http (6)，本环境未实跑。
- 无删除/过期迁移：设计明确不清理暂存（B-01），不适用。

### 4. 输入域边界

- 大小 0、恰好上限、上限+1；有/无 contentLength；多块流超限：uploads (1)/(3) 通过。
- 文件名空串、点/双点、路径穿越、反斜杠、控制字符、前导点、Unicode 多字节、200 字节与超长：uploads (4) 通过。
- UUID 大写/非法/不存在/partial；目录 0/1/2 文件：uploads (1)/(5) 通过。
- 个数上限本轮只建立共用常量，创建表单校验属于 M-04，不适用本轮单文件端点；日期、闰日、跨年不涉及上传域。

## 交接

审计首先复验完整 37 用例及 HTTP 两项负向对照。待审依据为设计明确允许的「31 个非监听通过 + 6 个 EPERM」情形，不代表全验收绿色。宿主仍需跑快检与合并前门禁。
