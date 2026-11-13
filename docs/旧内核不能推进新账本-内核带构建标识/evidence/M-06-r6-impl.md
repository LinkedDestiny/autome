# M-06 · 实现 #6

## 改动与验收

新增 package-cli.sh --binary/--out；缺省仍构建 release 并输出 dist，复制安装及校验和路径不变。新增两个真实打包/安装测试；README、技术设计 §8.1/13/14、计划 §7.3 补齐构建标识、只读拒绝、逃生口和先停旧进程的升级顺序。没有改动 M-07。

- 执行设计 M-06 验收命令：cargo build -p autome-cli && npm run test --prefix apps/web -- test/install-alignment.test.ts，随后契约列出的六条 grep 检查。构建成功，2/2 用例通过、0 失败、0 跳过（Darwin）；六条文档检查通过。原始输出：.autome/output/m06-acceptance.txt。
- 用例总数 2，相对上轮 M-05 的 51 为 -49：本轮仅选安装测试文件，上轮八个文件的 51 例未重跑；仓库新增 2 例，没有删除用例。M-06 同范围此前无测试，基线 0 → 2。另有 6 项参数边界检查，不计入 Vitest 用例数。
- sh -n scripts/package-cli.sh、定向 eslint apps/web/test/install-alignment.test.ts、git diff --check 通过。未运行全量门禁或宿主快检。
- 上轮审计 M-05 通过，没有审计退回的缺陷检查需要搬运或红绿复现。

## 标待审前自审清单

1. **情形表逐行**：方案 13 步骤 1 的 tar 文件、README、SHA256 由第 1 例核对；步骤 2 的空 PREFIX + 隔离 AUTOME_HOME 安装且校验通过、步骤 3 的 PATH 解析结果、步骤 4 的真实 CoreSupervisor 握手与 CLI 标识一致由第 2 例核对。两个测试均通过；PATH 不含 cargo，若忽略 --binary 会直接失败，--out 错误会找不到临时 tar。文档逐项对照方案 14，包含已打开旧连接不会被迁移撤销、bootout 在安装 doctor 之前，以及重打包桌面端。
2. **失败分支**：新增参数缺值（--binary/--out）、空值、未知参数均退出 2；不存在的 binary 退出 1，六项实跑通过，输出 .autome/output/m06-boundaries.txt。授权、网络不可达、服务超时不适用：此次仅本地打包安装，未新增这些分支；既有下载实现未改。
3. **全部迁移**：空安装目录 → 包安装 → PATH 解析 → starting → connected → env.get 的链路实跑通过，测试 teardown 停止子进程并清理临时目录。此次无新增内核状态迁移；已有 outdated/接管状态机属于已审里程碑，未变更。安装前 PREFIX 不存在断言通过。
4. **域边界**：路径含空格（临时根目录 autome install 前缀）、相对 binary、绝对 out、空 PREFIX、单个 tar 均实跑；空参数及缺值拒绝见六项检查。日期/整数上限不适用，新增输入域仅文件路径。全部命令使用临时 home、prefix 和显式临时 DB，未接触真实账本或安装位置。

## 结论

M-06 验收通过，提交独立审计，标为待审。实现轮计数 6/35。
