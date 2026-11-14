# M-02 · 实现 #3

结论：实现与定向静态检查完成；验收 3 条在启动前被沙箱 `spawnSync ps EPERM` 阻塞，保持 **开放**，不标待审。implementation-round 为 3/65，reopen 不变。未跑全量门禁、未启动下一会话。

## 改动

- main 在解析 Args、打开 Client 前分发 web；web.rs 查找 Node 与四档面板目录，exec 原样传参、绑定自身内核路径、透传退出码。
- 登录 PATH 逐函数迁移，保留标记、3 秒超时、fish 查询、回退目录；runFromEnv 将 PATH overlay 传入内核环境。桌面端原文件保留。
- service install/uninstall 生成 com.autome.web plist，原子替换、先 bootout 再 bootstrap；固定 Node 绝对路径与 home/root，成功退出不重启；其它 label 与非 macOS 拒绝。
- 测试均使用临时 home；自启动只写临时目录、调用假 launchctl。新增额外两条失败分支测试。自审修正 HTTP 验收回包字段为现有 gateway 的 payload；迁移代码的冗余初值按 lint 移除。
- 上轮审计通过 M-01，无退回 M-02 的缺陷或需要逐字迁入的审计判别检查。

## 命令与结果

原始输出在 `.autome/output/m02-*.txt`，未复制进任务目录。

- `cargo test -p autome-cli --test cli web_`：7/7 通过（m02-cli.txt）。
- `cargo test -p autome-cli --bin autome usage`：4/4 通过（m02-usage.txt）。
- `cd apps/web && npx vitest run test/login-path.test.ts test/login-path-real.test.ts test/web-service.test.ts`：24 条，21 通过、3 失败、0 跳过（m02-web-final.txt）。首次运行 22 条为 19 通过、3 同因失败；新增两个失败分支用例后为 24 条。
- `npm run lint --prefix apps/web`、`npm run typecheck --prefix apps/web`：最终通过（m02-lint-final.txt、m02-typecheck-final.txt）。
- `cargo clippy -p autome-cli --all-targets -- -D warnings`：通过（m02-clippy.txt）。仅本包检查，不是宿主全工作区快检。
- 已对变动 Rust 文件运行 rustfmt；`git diff --check` 通过。

**不同用例总数 35 = 7 + 4 + 24；32 通过、3 失败、0 跳过。** 相比上轮实现记录的 84 条减少 49，原因是本轮切到 M-02 集合，不重复 M-01 的 69 条验收和 15 条附加检查；不是删测试或条件跳过。M-02 同集合的代码基线只有 3 条 usage，本轮新增 32 条（7 CLI、1 usage、13 迁移 PATH、2 集成、9 service）。相对设计预期 33 条增加 2，来自授权/bootstrap 失败与输入/写盘失败检查。

未完成的三个用例（都在 harness 的 ps 预检失败，尚未生成服务子进程）：
1. login-path-real：最小 PATH 经启动器与登录 shell 后 env.get 找到 claude/codex。
2. login-path-real：登录 shell、服务与内核无残留。
3. web-service：serve 复用活实例以 0 退出。

## 标待审前自审清单（本轮未满足，保持开放）

1. **情形表逐行**：A1 参数原样（含 --force、-x、空格）、自身 core、不打开账本、缺 Node、缺面板、目录优先级、退出码七行均通过。A9 标记/杂音、缺标记/空值、去重/空段、zsh/fish、登录 PATH 优先、抛错与无输出回退、无 HOME、Windows、相同 PATH 无 overlay、GUI overlay 共 13 条通过。A7 plist 内容及 plutil、重复安装顺序、重复卸载、label 限制、平台拒绝、默认目录解析六行通过；serve 已有实例一行受 ps 阻塞。真实 env.get 与无残留两行受同一环境阻塞。
2. **失败分支**：缺 Node/面板通过；shell 抛错、无输出、空 PATH 回退通过；launchctl bootout 未加载忽略、授权失败向上传播、bootstrap 失败向上传播通过；写盘目标为普通文件时拒绝通过。外部网络服务不可达：不适用，本里程碑不调用远端服务。shell 3 秒超时保留原实现，尚无本轮真实超时实验；真实服务相关失败行为不能由纯函数结果替代。
3. **全部迁移**：未安装→安装、已安装→重装、已安装→卸载、未安装→卸载均通过；bootout 失败保留文件、bootstrap 失败向上传播通过。终端 PATH→无覆盖、GUI PATH→覆盖、shell 失败→回退均通过。活实例→serve 正常退出的真实迁移被沙箱阻塞；不声称完成。
4. **输入域边界**：CLI 参数空格/未知开关、PATH 空段/重复段、无 HOME、空标记内容、XML 特殊字符（home 含 &）、空 service 动作、空 core 路径、错误 label、非 macOS 均通过。日期/前导零/最大数值：不适用，本里程碑无相应输入域（端口归 M-01）。

## 后续

宿主或具备 ps/监听权限的会话复跑 M-02 原验收命令，取得上述三条真实用例证据后才可标待审。没有修改测试为跳过，也没有把环境阻塞记成协议失败。
