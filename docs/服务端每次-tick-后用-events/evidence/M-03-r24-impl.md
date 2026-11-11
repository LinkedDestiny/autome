# M-03 · 实现 #24

本轮只推进 M-03（R-04）。上一轮 M-02 审计通过，没有发现实现缺陷的判别检查需要逐字迁入、先红后绿。M-04 的 NUL 修复未动。

## 实现与修复

- 新增 live-client：hello 检查点、resume、同轮事件合批与去重、reload 清检查点及旧事件批、core 回调、500–10000ms 指数退避、stop 清定时器。构造连接失败进入重试。
- 新增 live-routes：kind/subject 匹配器，按 href 值识别纯重验；在途事件批取并集，仅最新重验 settle 后清理；导航与表单遵循 Router 默认行为。
- live.tsx 在 effect 内连接同源 ws/wss，返回连接和内核状态；root 接中文顶部状态提示，SSR 不连接。没有新增业务页面或 loader 数据副本。
- 测试首次运行的路径错误已修正为 import.meta.url 定位；按项目 NodeNext 配置补全导入扩展名。此为本轮自测修正，不是上一轮审计退回。

## 验收命令与结果

各命令独立运行，原始输出位于 `.autome/output/m03-*.txt`。

| 命令 | 退出码 | 结果与证据 |
|---|---|---|
| `npm run lint --prefix apps/web` | 0 | m03-lint.txt，无错误警告 |
| `npm run typecheck --prefix apps/web` | 0 | m03-typecheck.txt |
| `npm test --prefix apps/web` | 1 | m03-web.txt：22 文件、224 用例，197 通过、11 失败、16 跳过；3 文件因监听权限失败 |
| `npm run build --prefix apps/web` | 0 | m03-build.txt：SSR 与浏览器 bundle 构建通过，客户端仅 type 导入协议 |
| `npm test --prefix apps/desktop` | 0 | m03-desktop.txt：104 总数、103 通过、1 原有跳过 |
| `git diff 30e5d51 --stat -- crates apps/desktop protocol plugins` | 0 | m03-boundary.txt 为空 |
| `npm exec --prefix apps/web -- vitest run --root apps/web test/live-client.test.ts test/live-routes.test.ts test/live-banner.test.ts` | 0 | m03-focused.txt：3 文件、16 用例全通过、0 跳过 |

**基线差值：**上一轮审计实际 Web 208 个，本轮 224，增加 16，来自 live-client 8、live-routes 6、live-banner 2；没有删除或缩减已有用例。比设计契约 219 多 5，全部继承上一轮的 208 vs 203 差额。Desktop 104 vs 104，差 0。

**环境阻断的准确范围：**gateway-http 11 个失败；eventbus-live 7 个、server 9 个因启动钩子失败未执行，合计 27。根因是 `listen EPERM: operation not permitted 127.0.0.1`，同时有 `File watcher error: EMFILE`。没有添加 skip，未声称真实 HTTP/WS、首页开发/生产 SSR 已验收。按任务文件“实现轮被环境阻断时”约定提交待审，由可监听环境独立复验。H-01～H-03 仍是既有人工验收，浏览器视觉截图未取证；未修改验收契约。

## 标待审前自审清单

### 1. 情形表逐行

| 设计 §4 情形 | 检查与结果 |
|---|---|
| 首次 hello 记 seq，不 resume | client ①通过；hello 同时更新 core 与 open |
| 有检查点重连 hello 发 resume | client ②通过，after 为最后已收到事件 seq |
| 无检查点且此前已连接 | client ⑧通过：hello 重建检查点并 onReload，不发 resume |
| seq <= lastSeq 丢弃；其它交 onEvents | client ④通过；同轮两条合并为一次回调 |
| reload 清检查点并整体重验 | client ⑤⑧、routes ⑤通过，在线换账本及 resume 后换账本均覆盖 |
| core 单独回调 | client ⑤通过；离线原因原样传递，不触发事件回调 |
| 断线退避、hello 归零、stop | client ③⑥⑦通过 |
| task kind+subject 精确选择 | routes ①通过，T-1 不刷新项目与 T-2；T-2 对照亦通过 |
| project/rule 前缀选择 | routes ②通过，project 只刷新项目 |
| 新旧事件批在途并集 | routes ③通过，前后两批 loader 都提交最新值，settle 后普通重验恢复 |
| href 值相同纯重验、导航/表单默认 | routes ①④通过，真实 createMemoryRouter 执行 |
| reload 全路由 | routes ⑤通过 |
| loader/clientLoader 接线守卫 | routes ⑥通过：报告坏 fixture 文件名，root/routes 实际扫描通过；扫描逻辑同时识别两种声明 |
| SSR 与提示 | banner 2 用例通过：reconnecting 输出中文 role=status、top:0，其余空输出；build 通过，真实 server SSR 受环境阻断 |

### 2. 失败分支

- 外部连接不可达：client ⑥构造器抛错，500ms 后重试；初次连接未 hello 就关闭也进入 reconnecting。
- 连续连接失败：client ③逐次执行 500/1000/2000/4000/8000/10000/10000ms，临界前不重连、临界时重连；hello 后回 500ms。
- 空事件批/未知 kind：routes ②执行，loader 计数不变。
- 拒绝授权：不适用 M-03，W2 升级守卫属于已完成服务端里程碑；本轮不更改它。
- 读盘失败、业务超时：不适用，浏览器客户端不读盘，设计 §4 没有独立握手超时机制；网络断开按退避处理。
- 重验失败清理：实现使用 finally 按 generation 清理；本设计没有点名失败后的额外产品策略，不宣称新增重试功能。

### 3. 全部迁移

- connecting→open：client ①；connecting→reconnecting（连接构造失败、握手前关闭）：⑥。
- open→reconnecting→open：②⑥；reconnecting→reconnecting 连续失败与退避递增：③；open 后重置退避：③。
- connecting/open/reconnecting→stop：⑦，三种状态逐个验证不新建连接、不发待处理批。
- 检查点 null→hello.seq：①；seq→更大 seq、重复/更小不变：④；seq→reload→null→小 seq：⑧；null→断线→hello.seq+整体重验：⑧。
- onEvents 合批→纯重验，未 settle→并入下一批→最新 settle 清理：routes ①③；空批期间导航/表单不改变默认：④；reload→all：⑤。
- WebSocket 服务端→浏览器 hello/event/reload/core 及浏览器→服务端 resume：client ①②④⑤⑧均检查。W4/W8 扩展消息不在本轮范围。

### 4. 输入域边界

- seq=0、重复、倒退、Number.MAX_SAFE_INTEGER：client ④通过；seq=hello.seq 丢弃：①⑧。
- 单事件、同轮多事件、空事件批、空 kind/subject、未匹配前缀、T-1/T-2：routes ①②与 client ④通过。
- 退避临界 delay−1/delay、上限前 8000、达到 10000 及继续失败仍封顶：client ③通过。
- 日期/闰日/跨年、前导零字符串：不适用，设计协议 seq 是 JSON 数字，M-03 不接受日期或数字字符串输入。

## 负向对照

执行 `.autome/output/m03-mutations.mjs`，每次只修改一处源码，finally 原样恢复；日志 m03-negative-*.txt。

| 注入 | 退出码 | 实际捕获 |
|---|---|---|
| 不设置事件批直接重验 | 1 | routes ①无关 loader 增加 |
| 调用 revalidate 后立即清事件批 | 1 | routes ③前一批 task 未取得版本 2 |
| URL 用对象相等替代 href | 1 | routes ①无关 loader 增加 |
| 退避去掉 10000 上限 | 1 | client ③封顶序列失败 |
| reload 保留旧检查点 | 1 | client ⑧小 seq 未被接受 |

还原后 16 个新增用例全绿。`git status --porcelain` 仅为本轮新增/修改的实现、测试与交付文档；本轮尚有未提交实现，不能伪称此时工作区为空。注入脚本的 finally 恢复原始字节，未留下任何注入差异。

结论：M-03 实现及无需监听的新增验收通过；全量的 27 个真实监听用例受沙箱限制，提交待审独立复验。
