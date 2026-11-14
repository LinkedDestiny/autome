# M-10 · 实现 #17

结论：已补齐四条浏览器用例与双服务夹具；验收被沙箱的 `spawnSync ps EPERM` 阻塞，M-10 保持开放，未标待审。没有修改内核、生产页面、协议或门禁配置，没有启动下一会话。

## 改动

- `flows.spec.ts`：D-1 从网页存草稿、开始、实时换列、铃铛等你批准、网页批准、合并到 done；监听 document load，必须恰好一次。
- D-2 用真实 CLI `task new --kind bug --brief -` 建卡；假 CLI 三个会话分别等待浏览器释放，确保定位、复现检查、修复、复核是逐轮出现；检查节点顺序、复现非零结果和合并后的 done 列。
- `live.spec.ts`：D-3 独立服务明确为 3000ms tick，从真实 CLI approve 返回开始计时，五秒内离开 needs_you 且进入 running；D-4 杀本测试包装器记录的内核 PID，看到 offline 后 CLI 建草稿，恢复后实时出现且没有刷新。
- 复用 w7-flow 的真实协议步骤，按任务 slug 隔离脚本；移除源夹具里与本链路无关的 Backlog/争议条目。假 CLI 等待有 60 秒上限。
- 全局 setup 启动两个独立临时 HOME/AUTOME_HOME 服务；实时服务内核包装器在重启前延迟三秒。teardown 对两个根目录分别停任务、停服务、清残留并断言；错误保留底层原因。

## 命令与结果

所有原始长输出在 `.autome/output/`，未放入任务文档目录。

| 命令 | 结果 | 输出 |
|---|---|---|
| `cd apps/web && /usr/bin/time -p npm run e2e` | 两次均在 setup 第一次进程扫描失败；最终 `spawnSync ps EPERM`，real 0.73 秒。未启动服务、内核、假 CLI 或浏览器，不能作为性能或清理通过证据 | `m10-e2e.txt` |
| `cd apps/web && npx playwright test --list` | 20 条 / 4 文件，发现成功 | `m10-list.txt` |
| `cd apps/web && npx vitest run test/e2e-preflight.test.ts` | 3 条通过、0 失败、0 跳过 | `m10-preflight.txt` |
| `cd apps/web && npx eslint e2e --max-warnings 0` | 通过，无输出 | `m10-eslint.txt` |
| `cd apps/web && npx tsc --noEmit` | 通过，无输出 | `m10-tsc.txt` |
| `git diff --check` | 通过 | 无输出 |

用例总数：浏览器发现 **20** 条，相对 M-09 审计的 16 条 **+4**（D-1/D-2/D-3/D-4 各一条）；本轮实际执行浏览器 **0** 条，20 条全被 setup 阻塞，并非跳过或通过。预检单测 **3** 条，与其上一轮基线差 **0**。本轮所选总范围 23 条，对比 M-09 的 49 条（33 vitest + 16 浏览器）净 **-26**：没有重跑无关的 core-sidecar/ci-config 共 30 条，新增浏览器 4 条。实际只执行了 3 条预检单测。未运行全量门禁及宿主快检组合命令。

## 标待审前的四项自审（本轮未满足标待审条件）

1. **情形表逐行**
   - D-1：草稿 todo → running → needs_you，铃铛通知、批准、合并、done 与 load=1 均已写断言；执行被 ps 权限阻塞。
   - D-2：CLI Bug 建卡，定位 → 复现检查（已复现）→ 修复 → 复核 → 合并 → done；每个会话用释放文件控制，确认未来节点尚未出现，均已写断言；执行被阻塞。
   - D-3：独立 HOME 服务、3000ms tick、CLI approve 后五秒内换列并无 reload，已写断言；执行被阻塞。
   - D-4：SIGKILL → offline → 离线建卡 → 恢复 → todo 新卡且 load=1，已写断言；执行被阻塞。
   - D-7：20 条发现通过；170 秒 globalTimeout 未改，实际整套耗时 ≤180 秒与残留检查未取得通过证据。
2. **失败分支**
   - 设计点名的内核异常退出/离线恢复由 D-4 覆盖，尚未实跑。
   - 夹具启动失败：实际执行并复现 ps EPERM，发生在 spawn 之前，日志明确原始错误；清理检查同样因 ps 权限失败，未掩盖为成功。
   - 拒绝授权、读盘失败、空输入、外部服务不可达：本里程碑不新增对应产品分支，不适用；未据此声称现有产品已覆盖。
   - 设计负向对照（整页 reload、缺失 events.since 补齐、错误的 1000ms live tick）均需要浏览器执行，环境阻塞，未做注入，未声称通过。
3. **全部迁移**
   - 需求：draft/todo → running → await_design_approval/needs_you → running → await_merge → 合并 → done。
   - Bug：locate → repro_check → fix → fix_gate → fix_review → await_merge → 合并 → done；显式 UI 断言覆盖设计要求的四节点及 done，其余由真实内核驱动。
   - 通道：网页创建/开始/批准/合并 → 内核 → WebSocket → 页面；CLI 建 Bug/批准/离线建卡 → 账本 → 服务 → 页面；在线 → offline → 在线补齐。用例均已实现，全部运行阻塞，不能称已验证。
4. **域边界**
   - 本里程碑未定义日期/数值输入域，前导零、闰日、跨年、最大值不适用。
   - 时序边界明确为 3000ms tick、5000ms 更新、重启前 3 秒延迟、整步 180 秒；已写断言/维持配置，运行被阻塞。
   - 空目录缺浏览器、缺构建、依赖齐全三种预检输入实际执行，3 条通过。

## 审计回归与后续

上一轮 M-09 审计通过，没有退回 M-10 的实现缺陷，不存在本轮必须逐字搬入的红绿检查。下一轮首先在能执行 ps/监听/启动 Chromium 的宿主环境运行原验收命令，解决实跑发现的问题；20 条全过、残留检查通过、real ≤180 秒后才可标待审。
