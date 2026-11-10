# M-04 · 实现 #5

结论：实现已落地，验收被本会话环境阻断；M-04 保留「开放」，不计 reopen。未标待审、未关闭里程碑。

## 改动

- 新增 ESLint flat config，启用 JS / TypeScript recommended，忽略 build 与 .react-router；增加 lint / typecheck 命令，沿用既有严格 tsconfig。
- ci-local 在 contract 后、package 前新增 Web 节：缺 node_modules 才 npm ci，四个检查独立 run。
- ci.yml 新增 macos-14、15 分钟限时的 web job，Node 22、npm 锁文件缓存及五个工作目录为 apps/web 的安装/检查步骤。
- 新增 ci-config.test.ts 的 6 个契约用例。上轮审计 M-03 通过，无退回缺陷，因此没有须逐字搬入的审计回归检查。

## 验收命令与结果

1. `npm run lint --prefix apps/web`：exit 0；原始输出 `.autome/output/m04-lint.txt`。
2. `npm run typecheck --prefix apps/web`：exit 0；原始输出 `.autome/output/m04-types.txt`。
3. 约定验收中的 `rm -rf apps/web/node_modules` 被命令工具拒绝；改用 `mv apps/web/node_modules .autome/output/m04-node_modules-before-install` 保留原目录，等价建立缺依赖的首次安装条件，再执行 `sh scripts/ci-local.sh > .autome/output/m04.txt 2>&1`。
   总退出码 1，用时 78 秒。Format、Clippy、Test、End to end、Adapter flags、Desktop、Contract、Web install、Web lint、Web typecheck、Web build 均 ok；Web test FAILED。Desktop 依赖原已存在，未触发 Desktop install；未运行可选 package。
   Web 耗时：install 2 秒、lint 1 秒、typecheck 2 秒、test 4 秒、build 1 秒，远低于 30 分钟门禁预算。
   Web 共 5 文件、38 用例（上一轮 4 文件、32 用例，差 +6，全部来自新增 ci-config）；31 通过、7 跳过。server.test.ts 的启动钩子失败，错误为 `listen EPERM: operation not permitted 127.0.0.1`，并伴随文件监听 `EMFILE`；7 个服务用例未取得通过证据，不能以其他 31 个通过替代。
4. 独立最小环境检查：用 Node `net.createServer().listen(0, '127.0.0.1')`，错误回调设置非零退出码。exit 1、EPERM，原始输出 `.autome/output/m04-listen.txt`。不依赖应用代码即可复现监听限制。当前权限策略禁止提权，故未尝试绕过。

## 标待审前的四项自审（已执行，因验收阻塞不标待审）

1. **情形表逐行**：设计 §5 的 lint 已通过；typegen + tsc 已通过，既有 tsconfig 覆盖 app/server/scripts/test 和 TS 配置，checkJs 覆盖入口 JS；vitest 独立配置与服务进程用例沿用。§6 的 job 平台/限时、Node 22/npm ci、四步齐全、全部 job 限时、缺依赖才安装、四行独立 run 位于 package 分支外分别由 6 个新用例检查，通过。首次安装条件已实跑；已安装条件由第二次 ci-local 验证。Electron 探针不接入门禁。
2. **失败分支**：本里程碑点名 lint 失败后继续其他步骤、四步合链时契约失败，检查记录见下节。拒绝授权、读盘失败、超时、空输入和外部服务不可达不属于 M-04 设计点名的新增分支，不适用。服务监听被拒绝按真实环境阻塞保留，不弱化断言。
3. **全部迁移**：无产品状态机或多通道，不适用。CI 的有/无依赖目录两种分支分别通过首次安装与第二次不安装覆盖；成功和失败后的汇总路径由正常/负向运行覆盖（正常运行仍受服务监听限制，未取得总退出 0）。
4. **域边界**：无新增用户输入域，不适用。CI 固定配置中的 Node '22'、正数 timeout、四项命令集合与工作目录均由配置契约检查。

## 后续

在允许本地监听且文件监听资源可用的执行环境，重跑 M-04 完整验收，需得到 Web 38/38 和 ci-local exit 0 后才标待审。未执行 GitHub Actions（仓库无 remote）；不能把本地配置检查当成托管 runner 实跑。

## 负向对照与恢复

- 在 `app/routes/home.tsx` 临时加入 `const unusedM04 = 1;`，执行完整 `sh scripts/ci-local.sh`：exit 1，73 秒；Web lint 报 no-unused-vars，typecheck ok（2 秒），test 确实运行但仍因环境导致 31 通过/7 跳过（3 秒），build ok（1 秒）。未出现 Web install，验证已安装分支。原始输出 `.autome/output/m04-negative-lint.txt`。该检查证明 lint 失败不遮蔽后续步骤，但不声称后续测试通过。
- 临时把四行 Web run 合为一条 `&&` 链，执行 `npm test --prefix apps/web -- test/ci-config.test.ts`：exit 1，6 用例中 5 通过/1 失败，失败为独立四行契约。原始输出 `.autome/output/m04-negative-chain.txt`。
- 两处注入均已恢复。相同配置测试命令 exit 0，6/6；lint 与 typecheck 重新 exit 0。输出分别为 `.autome/output/m04-config-restored.txt`、`m04-lint-restored.txt`、`m04-types-restored.txt`。配置子集为全量 38 用例中的 6 个，不是新增或取代全量基线。
