# M-05 实现 #5

结论：实现与自动化用例已落地；验收命令未全绿，M-05 保持开放，reopen 保持 0。阻塞是沙箱禁止监听本地端口，不是协议失败。未推进 M-06，不启动下一会话。

## 实现

- 内核查找改为环境覆盖 → 包内 automed → PATH 第一个 autome → 默认安装 autome → cargo，透传 core 参数。
- 启动前获取构建标识；缺失文件、非零退出、空/旧输出和超时均产生可解释日志，不阻止后续启动。设计补充了原先遗漏的超时文案。
- outdated 经 supervisor、WebSocket hello/core、live-client 到横幅与写闸门，保留内核原话；读请求仍可执行。
- outdated 每 10 个 tick 间隔检查一次，重复拒绝不重复记录；恢复记录一次。重启先停止并回收旧进程，强杀也等待 exit；等待回收时收到 stop 会取消启动。
- 原有假内核允许空 payload，维持该兼容性；hello 新增 reason 后同步原有事件续传测试的完整对象断言。

## 命令与结果

1. 改动前基线：`npm run test --prefix apps/web -- test/core-binary.test.ts test/ticker.test.ts test/core-supervisor.test.ts test/live-banner.test.ts`。27 个，全部通过。原始输出 `.autome/output/m05-baseline.txt`。
2. 最终验收命令（逐字沿用契约）：

   `cargo build -p automed -p autome-cli && npm run test --prefix apps/web -- test/core-binary.test.ts test/kernel-identity.test.ts test/older-kernel-live.test.ts test/older-kernel-core.test.ts test/older-kernel-real.test.ts test/ticker.test.ts test/core-supervisor.test.ts test/live-banner.test.ts`

   cargo build 成功；51 个用例，49 通过、2 失败、0 跳过。两例均为真实 Web 服务启动时报 `listen EPERM: operation not permitted 127.0.0.1`，尚未执行到 HTTP/WebSocket 断言。原始输出 `.autome/output/m05-final-accept.txt`。未删除、跳过或弱化这两例。
3. 直接相关回归：`npm run test --prefix apps/web -- test/live-client.test.ts test/eventbus.test.ts test/gateway.test.ts`，43 个全部通过。输出 `.autome/output/m05-related.txt`。
4. 诊断性子集：`npm run test --prefix apps/web -- test/older-kernel-real.test.ts -t '真实 stdio'`，1 通过、2 被名称过滤；该例及后来新增的强杀回收例均已被最终验收重新包含，不重复计数。输出 `.autome/output/m05-stdio.txt`。
5. `git diff --check` 通过。未运行全量门禁或宿主快检；未运行安装脚本、未接触真实 AUTOME_HOME。

用例总数：最终验收 51，相对本轮改动前相同已存在文件基线 27 增加 24：查找测试 +3、identity +7、live +5、core +5、real +4。相比设计原定 48 增加 3，分别是超时、真实 stdio 恢复、强杀回收。加上直接相关回归，本轮最终去重范围 94 个，92 通过、2 环境失败。上一轮 M-04 验收是 8 个 CLI 用例；数值差 +86 来自切换里程碑与新增 Web 覆盖，不表示 CLI 用例被删除或本轮重新验证。

首轮 49 个里有 3 失败：两例 EPERM，加上空 payload 的握手兼容失败；已修复后最终原有 core-supervisor 10 例全过。扩展回归最初发现 hello 新 reason 的期望差异，更新契约断言后 43 例全过。上一轮审计通过且未退回缺陷，因此“逐字搬入审计判别检查并红绿复现”不适用。

## 自审清单（未据此标待审）

1. **情形表逐行**
   - 环境覆盖优先、不查盘、autome 带 core：core-binary 通过。
   - bundle 优先且保持包内路径：core-binary 通过。
   - PATH 首个 autome、默认安装兜底、旧 local-bin automed 不再查找：core-binary 通过。
   - cargo 默认/debug/release、空覆盖：core-binary 通过。
   - cli/cargo 日志路径、标识、来源；缺失、非零、无标识、超时：identity 7 例通过。
   - outdated 映射、hello 原因端到端回调、横幅原文、写闸门、禁止 reload：live 5 例通过。
   - 读放行、慢 tick、进入/恢复日志、先回收再重启：core 5 例通过。
   - 真实 stdio 可读/拒写/逃生口后恢复、强杀回收：real 两例通过。
   - 真实 HTTP 200/409、WebSocket hello、startServer 日志/参数：两例 EPERM，未取证。
2. **失败分支**
   - 文件不存在、version 非零、输出空/旧格式、version 超时：均已运行通过。
   - 内核启动失败、缺失二进制、三次失败转 offline、调用超时及迟到回复：原有 supervisor 用例通过。
   - 内核拒写：真实 stdio 返回 older_kernel 和相同消息；HTTP 409 未取证。
   - 外部服务不可达/授权弹窗：不适用，本里程碑只启动本地进程，无外部服务或系统授权协议。
3. **全部迁移**
   - starting → connected/outdated；connected → outdated；outdated → outdated（重复不记录）；outdated → connected（恢复一行）；outdated → stopped → starting → outdated：均有执行证据。
   - 原有 starting/connected → restarting/offline、offline → starting、connected restart 保持原进程、stop 取消后续 spawn、超时后迟到回复恢复：原有 supervisor 用例通过。
   - WebSocket hello/core 的 outdated 字段与 reason：映射和回调通过；真实 socket 未取证。
   - 非本轮状态（任务工作流状态迁移）：不适用，未修改内核任务状态机。
4. **输入域边界**
   - 未设 PATH、空覆盖、默认 profile/指定 profile、单个默认安装候选、多 PATH 候选、缺失文件、空 version 输出：通过。
   - 29999ms 尚不 tick、60000ms 恰好两次；重复消息不增日志、恢复后不再重复恢复日志：通过。
   - 日期/前导零/闰日/最大提交数的标识比较：不适用，本轮只展示已有内核标识，不实现解析比较规则。

## 限制与后续

H-01 视觉截图与真实按钮操作未取证，本轮沿用原有样式；本地监听受限，未开展浏览器验收。H-02/H-03 是合并后人工项，未执行。下一轮先在允许 localhost 监听的环境完整运行 M-05 验收，取得全绿再标待审。本轮没有请求提升权限（当前权限策略为 never）。
