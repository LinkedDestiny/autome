# 面板服务进程把环境变量暴露在进程列表里：`pgrep -fl autome-web` 能看到启动它的 shell 里的令牌

## 现象

2026-11-19 操作者在本机实测（面板由 `autome web start` 从一个带有 `ANTHROPIC_AUTH_TOKEN` 的 shell 启动）：

- `pgrep -fl autome-web` 的输出里，面板服务那一行在 `autome-web <AUTOME_HOME> serve` 之后跟着启动它的 shell 的环境变量，形如 `名字=值`，其中包含令牌的完整值。核对方式是把令牌读进 shell 变量后做子串比较，只打印是或否，结果为是。
- `ps -o command= -p <pid>` 只显示 `autome-web <AUTOME_HOME> serve` 三个词，不含环境变量。所以泄露出现在读取原始参数区的工具上（`pgrep -f` 一类）。

后果：任何把 `pgrep -fl` 输出写进日志、终端记录或问题报告的操作，都会把令牌带出去；本机其它能列进程的程序也可能读到。

线索（定位轮自行核实，不要照抄结论）：`apps/web/server/launcher.ts:134` 在启动时执行 `process.title = \`autome-web ${home} ${action}\``。Node 在 macOS 上改进程标题是覆写原始参数所在的内存；新标题比原来的参数短时，按参数个数往后数的工具会数进紧随其后的环境变量区。

## 复现

在隔离目录里做，不碰 `~/.autome`，不使用真实令牌：

```sh
npm run build --prefix apps/web
cd apps/web && npx vitest run test/process-listing-env.test.ts
```

`test/process-listing-env.test.ts` 是定位轮要新建的回归测试文件（见「线索与约束」）。

## 期望

1. 面板服务运行时，下面三种列法的输出里都不出现它的任何环境变量值：`pgrep -fl autome-web`、`ps -axo pid,command`、`ps -o args= -p <pid>`。
2. 面板实例的识别照常工作：`autome web status`、`autome web stop`、`autome web stop --force` 以及「旧实例已失效 / 暂时没有响应」的判断（`apps/web/server/instance.ts` 里依赖命令行包含 `autome-web` 的那两处）行为不变。若改了识别办法，这些路径要有对应的测试。
3. 面板带起的内核与会话拿到的环境不变。本缺陷只管「不出现在进程列表里」，不要顺手过滤传给内核与会话的环境变量：有用户靠 shell 里导出的密钥跑会话。
4. 面板带起的其它长驻子进程（内核 `autome core` 等）若有同样的问题，一并修；没有就在定位说明里写明核对过。

## 线索与约束

- **回归测试写进新文件** `apps/web/test/process-listing-env.test.ts`，登记进 `apps/web/test-groups.ts`。不要把断言补进已有测试文件（内核的「修正回归测试」流程目前只对新建的测试文件可用）。
- 用例做法：临时 `AUTOME_HOME`、空闲端口，启动面板服务时在环境里放一个哨兵变量（例如 `AUTOME_LEAK_SENTINEL=<随机串>`）；等 `/healthz` 就绪后，取这三种列法里属于该进程的输出，断言都不含哨兵值；再断言 `autome web status` 仍认得这个实例；最后停掉服务并确认没有残留进程。
- **先断言到达了场景再判断**：先确认服务进程存在、三种列法里都能找到它的那一行（含 `autome-web`），否则用例是空过的。
- 先红后绿：新用例在未修的代码上必须因 `pgrep -fl` 的输出含哨兵值而失败，把失败输出里的行数与命中位置写进定位说明（不要贴哨兵以外的任何环境变量值）。
- 任何输出、文档、证据里都不得出现真实令牌或其它真实环境变量的值；需要引用时只写变量名。
- 复核轮在沙箱里跑不了 `ps` / `pgrep`，这是已知限制，记为验证缺口即可；该用例由宿主门禁实跑。
- 只改 `apps/web`（必要时含 `crates/autome-cli` 里拉起面板的那一小段）。不做截图。
