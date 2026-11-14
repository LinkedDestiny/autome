# 实现 #23 · 合并前 rebase（关联 M-02）

本轮没有开放里程碑：M-01 至 M-13 均已由审计关闭，上一轮 M-13 审计通过，无退回检查需要红绿迁入。按用户追加要求处理 rebase，不新增里程碑、不 reopen、不重新标待审；保留 status 已完成、current-milestone 无、next-action 无，只将 implementation-round 更新为 23/65。

## 修复与证据

- 默认分支 origin/HEAD 指向 origin/main；执行 git rebase main，目标 main 为 a762d3f8。110 个重放步骤完成，未 merge、未推送、未切换工作分支。
- a10992de 重放时 main.rs 尾部 operator_note 测试与 usage_lists_web 冲突；保留两函数并各自闭合、保留 test 属性。
- cli.rs 尾部 main 的 operator_note / status 测试与 WebFixture 及七条 web 测试冲突；保留两侧全部内容及各自闭合括号。
- git diff main -- 两个冲突文件显示只增加原有 Web 分发、帮助与测试，main 代码无删除；git merge-base --is-ancestor main HEAD 与 git diff --check 均退出 0。
- 原始输出：.autome/output/r23-rebase.txt、r23-rebase-continue.txt、r23-final-cli-diff.txt、r23-*.txt。

## 验收命令与结果

所有测试命令显式使用 mktemp -d 生成的 AUTOME_HOME，夹具另用自身临时目录。

- cargo test -p autome-cli --test cli web_：7/7。
- cargo test -p autome-cli --bin autome usage：4/4。
- npx --prefix apps/web vitest run --root apps/web test/login-path.test.ts test/login-path-real.test.ts test/web-service.test.ts：24 条，21 通过，3 失败，0 跳过。与 M-02 验收为同一集合。
- cargo test -p autome-cli --bin autome operator_note：3/3。
- cargo test -p autome-cli --test cli operator_note：1/1。
- cargo test -p autome-cli --test cli status：10/10，其中 operator_note_cli_records_and_status_displays_verbatim 与上一命令重复一次。

共执行 49 次用例，去重 48 条：45 通过、3 环境阻塞、0 跳过。较上一轮 M-13 审计 33 条增加 15 条，原因是本轮改验 CLI rebase 与 M-02 集合；没有新增或删除用例。M-02 同集合仍为 35 条，与实现 #3 基线差 0，附加 CLI 回归去重 13 条。

三条阻塞均为 harness 预检 spawnSync ps EPERM：最小 PATH 经启动器与登录 shell 后 env.get 找到 claude/codex；登录 shell、服务与内核无残留；serve 复用活实例以 0 退出。未改弱或跳过测试，不声称本轮 M-02 验收全绿，宿主需复验。未跑全量门禁或宿主快检。

## 四项自审清单

1. 情形表逐行：CLI 原样参数、自身内核路径、不打开账本、缺 Node、缺面板、路径优先级、退出码七行全部通过；main 原有 note 与 status 检查通过。登录 PATH/launchd 测试 21 条通过，上述三条真实进程检查受阻。
2. 失败分支：缺 Node、缺面板、原样非零退出码通过；note 缺文件、非法 UTF-8、互斥输入通过；shell 回退及 launchctl 失败由上述 Vitest 集合检查通过。没有新增外部服务失败分支。
3. 全部迁移：不适用，本轮仅合并两个文件尾部的独立测试，没有修改状态机或多通道行为；已有 service 集合仍执行，活实例复用迁移受环境阻塞。
4. 输入域边界：带空格参数、未知开关、空 PATH、不存在的 Node/面板、note 文件/标准输入/非法编码均由上述测试覆盖；日期与数值极限不适用，此次冲突不涉及这些输入域。

本轮结束后由 Autome 继续调度，未自行启动下一会话。
