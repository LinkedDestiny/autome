# 裁决记录

## 第 1 轮（design-round 0 → 1）

评审 #1 提出 3 条六类问题与 4 条非阻塞建议。逐条核对源码与设计后，3 条全部采纳并已在设计文档中修正；修正后没有剩余的六类问题，本轮把 `status` 改为 `实现中`，等用户批准。所有主张都是首次提出，复提计数均为 0，无争议项。

| ID | 主张摘要 | 设计位置 | 裁决 | 证据或理由 | 修改落点 | 复提计数 |
|---|---|---|---|---|---|---|
| D1-P01 | 新启动链 `serve → runFromEnv` 没有固定生产模式，没设 `NODE_ENV` 时进开发分支并加载包里不存在的 `vite` | A1、A5、A7、C1、M-03 | 采纳（内部逻辑矛盾） | 核对 `apps/web/server/main.ts:80`：`NODE_ENV === 'production'` 才选生产模式，否则开发；`http.ts` 开发分支导入 `vite`。精简包按设计不带 vite，反例成立 | A5 新增「生产模式」：`serve` 以 `runFromEnv({mode:'production'})` 起服务、不读 `NODE_ENV`；A1 `serve` 行、A7 plist 注释；M-01 web-launcher 第 1 条与 M-03 install-alignment 第 2 条在无 `NODE_ENV` 下验证，加两条负向对照 | 0 |
| D1-P02 | 单实例互斥没有覆盖直接 `serve` 的「判定—监听—登记」，launchd 与手工 start 交错会出现两个服务、一份登记；60 秒按龄接管活持有者也破坏互斥 | A5「竞争」、A7、C1 | 采纳（违反任务目标 A-2） | 交错反例成立：设计里 `serve` 只在写入前再检查一次，非互斥操作 | A5「竞争」重写：唯一持锁层是 `serve`，临界区覆盖判活→监听→内核就绪→登记；启动器父进程不持锁（无父子等锁）；活持有者不按年龄接管，死持有者经 `launch.lock.reap` 接管；`server.json` 用 `link()` 登记作后盾。M-01 增 web-instance 第 11、12 条、web-launcher 第 8 条（serve 与 start 交错），共 26 条，负向对照同步 | 0 |
| D1-P03 | 薄壳退出时只凭启动时的 `reused:false` 调普通 stop，会停掉用户后来起的继任服务；A8 超时 SIGKILL 并删文件同样没绑定实例 | C4、A8 | 采纳（内部逻辑矛盾） | 交错反例成立：stop 读到 S2 的合法 `server.json`，A5 校验通过 | A8 新增「绑定实例」：`stop --instance <uuid>`，信号、SIGKILL、删文件都绑定该次确认的 instance；A5 `start --json` 输出带 `instance`；C4 记下 instance、退出用 `--instance`；M-01 web-launcher 第 9 条、M-07 shell-main 第 9 条（desktop 总数 16→17） | 0 |

## 非阻塞建议的处置

不触发轮次；已顺手处理，不计复提。

- N-01 采纳：C1、C4、M-07 统一为「主进程只跑一次性 `start --json`，启动器再 spawn 分离的 `serve`」，shell-main 第 1、2 条的替身断言随之改写。
- N-02 进 Backlog：BL-07（M-08 实现注意）。
- N-03 采纳：M-05 clock 用例改成「四种情形表驱动、每个时区一条」，共 8 条（M-05 合计 41 条）；M-04 负向对照改为「不设上限时第 3 条失败」（`?done=all` 本就返回全部）。
- N-04 采纳：C3 要求 `href` 解析后同源、用 `loadURL`，M-07 第 7 条同步；BL-08 记复核提醒。

## 定稿判断

六类问题已清零：生产模式、单实例互斥、实例绑定三处矛盾都已在正文消除，M-01、M-03、M-07、M-08 的验收契约随之更新；里程碑表保持 10 个、各有验收命令、预期与负向对照。`status` 改为 `实现中`，`current-milestone` 置为 M-01。

提交由本轮结束时的 `git commit` 完成；若被拒绝则由 Autome 兜底。
