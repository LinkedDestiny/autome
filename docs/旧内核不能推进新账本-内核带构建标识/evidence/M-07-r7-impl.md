# M-07 · 实现 #7

## 实现与范围

CLI package 步骤创建一个临时根目录，下设不同的 bin 与 home；安装调用同时设置 PREFIX 与 AUTOME_HOME。EXIT trap 清理临时根，HUP/INT/TERM 转为退出并清理；set -e 保留安装失败退出码，不继续版本验证。步骤顺序不变。测试提取真实 sh -c 调用，用替身 package/install/autome 执行，不运行全量门禁或真实 CLI。
上一轮审计 M-06 通过，无退回缺陷或需逐字搬入的检查。

## 命令与结果

- 新测试首次运行：npm run test --prefix apps/web -- test/ci-config.test.ts，7 通过、1 失败，共 8；旧脚本没有隔离 AUTOME_HOME。输出：.autome/output/m07-red.txt。
- 测试提取方式调整为完整 sh -c 调用，使 shell 自行处理 trap 的引号，然后通过。
- 最终验收：npm run test --prefix apps/web -- test/ci-config.test.ts && sh -n scripts/ci-local.sh，退出码 0；8 通过、0 失败、0 跳过，语法检查通过。输出：.autome/output/m07-final.txt。
- 三项负向对照：删除 AUTOME_HOME 赋值、删除清理、安装失败改成 exit 0；分别退出 1，均为 7 通过、1 失败。输出：.autome/output/m07-negative-{no-home,no-cleanup,swallow-failure}.txt。全部还原后重新验收通过。
- 用例总数 8，相比本测试文件上一轮基线 7 增加 1：新增一个 shell 行为测试，内部跑成功及失败两种情形。上一轮 M-06 验收子集为 2，本轮子集为 8，表面差 +6 来自切换验收文件，不能视为新增 6 个用例。

## 标待审之前的自审清单

1. **情形表逐行**：安装成功（退出 0）与安装失败（退出 37）均实跑；两者 home/prefix 是临时根下不同目录，调用后目录消失；仅成功路径执行版本检查；调用者 home/prefix 未创建。既有 7 例仍通过，步骤顺序未变。
2. **失败分支**：设计点名的安装失败实跑并断言原退出码 37 与清理；三个负向对照均红。拒绝授权、读盘失败、外部服务不可达、空输入不适用，本里程碑不新增这些接口。
3. **全部迁移**：临时根创建 → 安装成功 → 版本检查 → 清理，以及创建 → 安装失败 → 清理 → 失败向上传递均实跑。无产品状态机或多通道迁移。
4. **输入域边界**：临时路径包含空格与中文，成功与失败均通过，验证变量引用；调用者预设 PREFIX/AUTOME_HOME 均被覆盖。日期、数值上下界与空串不适用，没有此类输入域声明。

未运行宿主快检、全量门禁和人工验收；按本轮授权仅执行 M-07 验收及负向对照。结论：M-07 待审。
