# M-03 实现 #3 证据

## 实现与范围

- `inputs.mjs keys` 输出按声明行对齐的键（准备行为空）；键组成与 `ci-local-key 1` 不变。
- `execute_prep` 成功且此前没有准备失败时原子替换生效键文件；重算失败写 `unknown`。准备失败只增不减地冻结重算，被 `--if-missing` 跳过不重算。
- `execute_step` 只为实际执行分支读取生效键，执行后仍复核输入；沿用分支、准备步骤准入、之前已经写出的报告不变。
- README 补充已装依赖树与刷新时机。未修改步骤清单、安装脚本、协议、schema 或其它里程碑。
- 上一轮审计 M-02 通过，没有靠检查发现并退回的实现缺陷，因此不存在需要逐字搬入的审计回归检查。

## 验收与数量

命令：`NO_COLOR=1 node --test scripts/test/ci-reuse.test.mjs scripts/test/ci-install.test.mjs && NO_COLOR=1 node --test --test-name-pattern='装依赖' scripts/test/ci-quick.test.mjs`。

结果：第一段 29/29，第二段 2/2，均 0 失败、0 跳过，共 **31 个**。同范围原有基线 23 个（reuse 18 + install 5，装依赖过滤 0），增加 **8 个**：R19–R24 六个、Q19–Q20 两个。循环内部的边界情形不另计为顶层用例。上一里程碑 M-02 的 61 个 Rust 验收用例属于不同范围，不能用作脚本用例基线。

初次输出：`.autome/output/impl3-accept-full.txt`、`impl3-accept-quick.txt`。最终补强后按同一验收命令重跑仍为 29 + 2 全过，输出 `impl3-accept-final-full.txt`、`impl3-accept-final-quick.txt`。
R22 补强重算失败后的恢复分支、R23 补强中间状态键精确相等断言后，分别单跑 `NO_COLOR=1 node --test --test-name-pattern='^22 ' scripts/test/ci-reuse.test.mjs` 与 `NO_COLOR=1 node --test --test-name-pattern='^23 ' scripts/test/ci-reuse.test.mjs`；输出 `impl3-recovery.txt`、`impl3-between.txt`，各 1 个，不增加独立用例总数。

## 标待审前的自审清单

### 1. 情形表逐行

- B-1 / R19：缺依赖首跑 Before/After A/After B 均 ran；After 键等于最终计划键，Before 不等；第二次仅 Before 执行、After 沿用且安装不执行；第三次全沿用。
- B-1 / Q19：真实 install-node.sh + 假 npm，首跑 lint/typecheck ran，第二次 reused，不执行 Web install。
- B-2 / R20：仅已装树变化、仅锁文件变化、依赖目录被删除，三种均不沿用。
- B-3 / R21：准备成功后步骤运行中改源码或锁文件，各报 partial。
- B-4 / R19、R23 及原 11：全部后继可沿用时准备不执行；即使前面一步需执行，也不调用 npm。
- R23：两次安装之间 Between 的键等于仅第一次安装状态的计划键、不等于最终键；After 等于最终键；重跑 Between 执行、After 沿用，npm 不调用。
- R24 / Q20：失败安装改依赖后，后继报 partial；无关成功准备不能掩盖失败；生成通过记录后重跑仍执行。R24 的失败未改输入对照照常 ran。

### 2. 失败分支

- 安装失败：R24、Q20，以及 install-fails 和原 reuse 17；整体非零，后续检查继续。
- 成功准备后的键重算失败：R22 暂移 `.git`，后续步骤恢复 `.git` 仍 partial；只有新的成功准备重算才能恢复 ran。
- 步骤中输入变化：R21 源码和锁文件两类，原 reuse 13 保持通过。
- 不可信通过记录、初始快照失败：原 reuse 07、14、18 覆盖工作区内/别名记录、缺失/损坏记录、损坏 index。
- 授权拒绝、外部服务不可达、超时：不适用；本里程碑只处理本地键时机，夹具 npm 不联网，也未新增授权或超时分支。

### 3. 全部迁移

- 初始计划键 → 成功准备后的键：R19/Q19；成功键 → 第二次成功准备后的键：R23。
- 初始键 → 准备失败冻结 → 无关准备成功仍冻结：R24；失败没改输入时可记 ran，改了输入时 partial。
- 成功准备 → 重算失败 unknown → 步骤复核恢复仍 partial → 后续成功准备重新得到可信键：R22 两种情形。
- 已有通过记录 → reused；旧依赖状态记录 → 执行 → ran → 下轮 reused：R19/R23。
- `--if-missing` 命中跳过：原 reuse 17；执行失败 → failed：原 reuse 09；运行中输入变化 → partial：R21。
- 完整门禁与快检均实跑（R19/Q19）。Q20 未另构造浏览器阶段，浏览器阶段共用的 execute_prep 失败后无关成功准备路径由 R24 覆盖；不声称执行了真实浏览器验收。

### 4. 输入域边界

- 无依赖目录/单次安装/两次安装：R19/R20/R23；空准备行与步骤索引间隔由 R19/R23 验证。
- 空通过记录、非法步骤名、损坏初始 index：原 reuse 18；带空格/中文路径、删除输入、忽略路径：原 reuse 15。
- 重算不可得的空行与 unknown 哨兵：R22 的两个后继步骤分别命中。
- 日期、前导零、闰日、跨年、数值最大值：不适用；本里程碑没有这类输入域。

## 负向对照

使用 `.autome/output/impl3-negative.mjs` 在各用例自己创建的临时仓库副本注入，不修改工作区产品代码。运行 `node .autome/output/impl3-negative.mjs`；汇总 `impl3-negative-summary.txt`。

- 禁止成功准备后重算：R19、Q19 各 1 个失败，实际 partial 而期望 ran。
- 不再读取已装依赖树：R20 1 个失败，变成错误沿用。
- 取消准备失败冻结：R24 1 个失败，实际 ran 而期望 partial。
- 将之前报告中的 ran 键改挂到最新键：R19、R23 共 2 个失败，Before/Between 与最终键错误相同。

合计 6 次预期失败，5 个命令均退出 1、无超时。对应未注入产品版本验收已绿；这些重复执行不计新增用例。

## 回归、快检与提交

- 指定快检：`sh scripts/ci-quick.sh --since feb9e0e8d88cbddc6468d2ffc067740577c3b4a3`，最终退出 0，执行 5 步、沿用 0、失败 0，用时 21 秒。输出 `.autome/output/impl3-host-quick-final.txt`。未运行项目全量门禁 `sh scripts/ci-local.sh`；测试内部运行的是临时夹具的步骤。
- 直接相关汇总器回归：`NO_COLOR=1 node --test scripts/test/ci-summary.test.mjs`，10/10 通过、0 跳过，较原有 10 个增加 0；输出 `impl3-summary.txt`。已完成且有汇总的验收与额外回归合计 41 个独立用例，较同范围基线 33 个增加 8 个；重复执行不重复计数。
- 设计要求的脚本回归：`NO_COLOR=1 node --test scripts/test/*.test.mjs` 已执行，但长时间没有最终汇总，主动中止；`NO_COLOR=1 node --test --test-timeout=120000 scripts/test/*.test.mjs` 再次长时间没有最终汇总，也中止。原始输出 `impl3-scripts.txt`、`impl3-scripts-bounded.txt`。没有完整用例总数，不能声称这一范围通过；不得把中止当作 0 失败。沙箱拒绝 `ps`（operation not permitted，`impl3-processes.txt`），停滞根因未定位。Web 依赖目录已存在，不能归因于缺依赖，未为此重新安装。
- 四项自审执行完成后才标待审；状态块保持实现中，implementation-round 为 3/15，M-03 待审，M-01/M-02 不变；状态字段与差异空白检查通过。
- 剩余验证缺口：全脚本回归未取得终结结果，交独立审计复验；人工 H-01/H-02 与宿主合并前全量门禁留在原定流程。
- 提交：使用当前任务分支，设置 NO_CI=1 避免提交钩子启动全量门禁；不推送、不启动下一会话。
