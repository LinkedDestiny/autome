# M-07 r9 实现证据

## 修复范围

仅修复 `apps/web/test/ci-config.test.ts` 的平台假设：不再要求 mktemp 的目录位于 TMPDIR 下，改为逐一断言传入的两个路径均不等于调用方 home/prefix；保留路径互异、实际目录存在、结束后消失、调用方目录未创建、版本命令是否执行以及退出码的断言。生产 shell 未改。已检查同一测试文件及 CLI package 片段，错误的路径前缀假设只有这一处。

## 审计检查与红绿证据

审计失败检查本来就在项目测试体系，无需另搬或复制用例；保留原用例及其余断言。

1. 原样执行 `npm run test --prefix apps/web -- test/ci-config.test.ts`，本环境 8/8 通过，与上一轮 macOS 结果不同。记录 `.autome/output/m07-r9-red.txt`（名称沿用原样复现步骤，实际不是红）。未声称本机原样复现。
2. 为复现审计指明的机制，通过 PATH 注入 mktemp 替身，用 `/usr/bin/mktemp -d /private/tmp/m07-r9.XXXXXXXX` 显式选择 TMPDIR 外的目录。其它命令与原用例未变。修复前原断言失败：7 通过、1 失败，总数 8；见 `.autome/output/m07-r9-portable-red.txt`。替身与命令记录在 `.autome/output/m07-r9-bin/mktemp`。
3. 修复后同一替身、同一用例 8/8 通过；见 `.autome/output/m07-r9-portable-green.txt`。此处替身只复现临时目录位置差异，不冒充一次真实 macOS 系统验收。
4. 验收命令 `npm run test --prefix apps/web -- test/ci-config.test.ts && sh -n scripts/ci-local.sh`：8 通过、0 失败、0 跳过，shell 语法退出 0。变异全部还原后再次运行同一验收亦通过，最终记录 `.autome/output/m07-r9-final.txt`。

用例总数 **8**，相对上一轮基线 **8** 差 **0**：只修改一个已有用例的断言，没有增删测试。重复运行与三次变异不算新增用例；安装成功和退出 37 是同一用例中的两个场景。未运行全量门禁或宿主快检。

## 标待审之前的自审清单

1. **情形表逐行**：M-07 没有独立情形表，按其落法逐项核对。临时 PREFIX 和 AUTOME_HOME 均由替身安装脚本确认存在；两者互异且均不等于调用方值；成功安装后执行版本命令；退出 37 时不执行版本命令；两条路径结束后均被清理；调用方目录均未创建。普通环境与 TMPDIR 外的临时目录环境全部通过。现有 7 个 CI 配置用例保持通过，步骤顺序不变。
2. **失败分支**：设计点名的安装失败已以退出 37 实跑，向上传递且完成清理。三条设计负向对照分别临时删除 AUTOME_HOME 赋值、删除 EXIT 清理 trap、安装失败后直接 exit 0：均为 7 通过、1 失败（总数 8），分别见 `.autome/output/m07-r9-negative-home.txt`、`m07-r9-negative-cleanup.txt`、`m07-r9-negative-swallowed.txt`。变异脚本为 `.autome/output/m07-r9-mutations.cjs`，finally 还原生产脚本，并清理记录下来的本轮临时目录。拒绝授权、读盘失败、外部服务等不适用：本里程碑未设计这些分支，使用本地替身。
3. **全部迁移**：无业务状态机；shell 两条流程均实跑：创建隔离目录→安装成功→版本命令→EXIT 清理；创建隔离目录→安装失败 37→直接退出→EXIT 清理。均符合契约。
4. **域边界**：临时工作路径含空格及中文，成功/非零退出码 0 与 37 已覆盖；临时根目录既在 TMPDIR 内也在其外均通过。日期、数字最大值、前导零等不适用：本里程碑没有这些输入域。

## 结论

M-07 可标待审；implementation-round 为 9/35。独立审计与宿主快检/合并前门禁待后续节点执行。
