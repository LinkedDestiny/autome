# M-05 · 实现 #5

范围：A 项内核接线、修测试定位守卫、隔离基线与 HEAD 成对检查、重新快照、CLI/RPC 可见性及文档。未推进 M-06；上一轮审计通过 M-04，没有需逐字搬入的退回缺陷检查。

## 实现与判据

- 冻结 fix prompt 的标记启用新结论；账本 requested / settled 推出次数和打开状态，上限沿用域层 2 次。修复和复核都能退回定位，定位不加修复轮次。定位 prompt 由 handoff 模块渲染边界与缺陷说明入口。
- 声明必须有非空的四项说明，定位结束必须删除声明。修复轮原保护不放宽；修测试定位只放行任务文档、整文件测试、support、新增测试、只改测试段的源文件。命令引用既有产品脚本不构成授权。
- 新快照合并旧快照、当前定位与任务早期累计测试；即使早期定位没有快照，其测试也会被后续修复守卫保护。普通定位原保护集基线保持原行为，C 项留给 M-07；普通定位只增加 support 记录。
- 一个异步 repro 作业内顺序跑隔离 merge-base 与 HEAD。临时检出位于项目 `.autome/output/repro-base/<任务>/<定位会话>/`，工作区维持成员布局。整文件按 Git blob 覆盖（保留二进制、可执行位），内联测试只拼接测试部分。缺显式依赖、不能识别测试段、基线读盘或检出失败均生成不可接受的基线结果。
- 退出码和超时标志进入 ReproPair；接受/继续修产品记录 settled，未复现保留模式。完成、停止及驱动恢复均清理临时检出；恢复时不删除仍在运行的成对作业。CLI 展示声明、成对结果和“定位（修正回归测试 n/2）”；task.get 增加 test_fix。

## 验收命令与结果

原始输出均在 `.autome/output/`，未转存到任务文档。

| 命令 | 用例数与结果 | 输出 |
|---|---|---|
| `cargo test -p automed -- test_fix_` | 16 通过（库 6 + e2e 10），0 失败、0 ignored | `m05-final3-automed.txt` |
| `cargo test -p automed --test end_to_end -- bug_` | 48 通过，0 失败、0 ignored | `m05-final-bug.txt` |
| `cargo test -p autome-cli test_fix_` | 2 通过，0 失败、0 ignored | `m05-final-cli.txt` |
| `grep -q '修正回归测试' README.md && grep -q '修正回归测试' docs/development/technical-design.md` | 两项均退出 0 | 与 CLI 验收串行运行 |
| `cargo test -p autome-domain test_fix_` | 9 通过，0 失败、0 ignored | `m05-domain.txt` |
| `cargo test -p automed --test end_to_end test_fix_e2e_missing_defect_section_is_rejected` | 加入空白值边界后重跑 1 通过（6 个子情形，不重复计数） | `m05-empty-field.txt` |
| `git diff --check` | 退出 0 | `m05-diff-check.txt` |

验收契约共 **66 个用例**，直接相关域测试另 9 个，共 **75 个不同用例**。不把 Cargo 为不匹配 target 报的“0 tests”计入用例数，也不把负向注入或重复执行重复计数。

上一轮 M-04 的命令基线为 71 + 40 = 111，本轮主验收 66，数值差 **-45**，因为从协议/金样范围切换到内核 Bug Loop 范围，不能解释为删除了 45 个测试。按本轮同一筛选范围比较上一轮 HEAD：既有 automed 标记 1 + Bug e2e 48 + CLI 上限提示 1 = **50**，本轮 **+16**（新库 5、新 e2e 10、新 CLI 1），没有删测试；域子集前后均为 9。设计原先估计的 Bug e2e 35 已按实际 48 校正，CLI 和 automed 计入 M-03/M-04 已新增且同样命中过滤器的用例。

## 标待审之前的自审清单

1. **情形表逐行：已执行。**
   - 任务文档：每条 e2e 的定位/修复/复核均通过真实文件提交推进。
   - 整文件测试可修：基线绿、HEAD 红等 e2e 修改已保护 regression.sh，允许进入成对检查。
   - support、内联测试、新增测试：A-V1 成功路径修正 expected.txt，保留产品前缀只改 lib.rs 测试段，并新增 tests/added.sh，基线退出 1 / HEAD 退出 0；修复轮数仍为 1。
   - 产品文件不可修、命令点名既有产品脚本不可修：两个独立 e2e 分别被可读守卫拒绝。
   - 旧快照与早期无快照的累计测试：专门 e2e 先无法复现（快照数 0），补充信息后定位，再修测试；后续修复改 tests/early.sh 被保护守卫拒绝。
   - 整文件覆盖/基线产品保留、二进制 support、内联拼接、检出移除分别有库检查。
2. **失败分支：已执行。**
   - 权限拒绝：产品文本、既有命令脚本两个越界 e2e；修复保护沿用既有 48 条 Bug 回归。
   - 缺陷说明缺小节或缺四项之一：同一 e2e 的 6 个子情形（含空白用例值）均失败且不记 requested；未删声明另有 e2e。
   - 依赖缺失：support 库检查先不覆盖 fixture.bin，必须输出不可接受的 127；覆盖后成功。
   - 读盘失败/无法识别测试段：内联库检查加入基线没有 new.rs、HEAD lib.rs 没测试标记两条负例，均输出“基线无法构造”且 127。
   - 基线检出失败：不存在的默认分支，构造结果为 127，无临时检出残留。
   - 超时/不能运行：相关域判定矩阵覆盖超时和 126/127，均不得接受为基线复现；没有用真实长时间睡眠模拟超时。
   - 外部服务不可达：不适用。本路径使用本地 Git 与用户的本地复现命令，不新增外部服务调用。
3. **全部迁移：已执行。**
   - Fix 声明 → Locate；FixReview 打回并声明 → Locate（核对 by=fix_review 和定位 prompt）。
   - 定位完成 → ReproCheck；基线红/HEAD 绿 → FixGate → FixReview → 合并卡点。
   - 基线红/HEAD 红 → 新 Fix（修复轮从 1 到 2）；基线绿 → Locate，连续第二次 → AwaitInfo，settled 仍为 0。
   - 第三次声明 → TestFixLimit，requested 保持 2；定位越界/未删声明/缺说明 → Protocol 失败。
   - 旧模板新结论被拒绝、不记 requested；旧结论既有 Bug 回归通过。域层 9 条补充验证状态与动作边界。
4. **输入域边界：已执行。**
   - 计数 0、1、2 的 used/open，settled 关闭模式；上限 2 后再次声明拒绝。
   - 退出码 0、非零、126、127、超时标志的域矩阵；空/缺说明走上述失败检查。
   - 空支持集、单个支持文件、二进制 fixture、测试段缺失、基线文件不存在均有子检查。
   - 日期、前导零及闰日：不适用，本里程碑没有日期/数字字符串输入域；纯域解析已有 M-03 用例保持通过。

## 负向对照与修正记录

- 把成对检查的 `git merge-base HEAD <默认分支>` 临时改成 `git rev-parse HEAD`，原样运行 `cargo test -p automed --test end_to_end test_fix_e2e_defective_fixture_is_corrected_and_reaches_merge`：**退出 101，0 过 1 败**（`m05-mutant-head.txt`）。还原产品代码后最终 test_fix 子集 **16 过**，相同成功路径恢复绿色。注入改动未保留。
- 首次基线绿 e2e 失败是测试夹具把下一定位轮写成了“复核通过”，内核正确拒绝；改为该轮合法的“待复核”后，同一检查通过。没有放宽领域断言或产品守卫。
- task.get 既有键基线只按设计 X-4e 移除 Bug 专属 test_fix 再比较，仍断言 feature/bug 其余键一致；既有行为测试未削弱。

## 边界与交接

未跑 `sh scripts/ci-local.sh`、宿主快检组合、clippy 或全量工作区测试；只运行本里程碑命令、相关域子集及格式化命令 `cargo fmt --all`。宿主在本轮结束后执行快检；H-01 模型付费行为验收仍留人工清单。本轮不推送、不启动下一会话，不改 `.autome/` 配置和固定协议副本。
