# main 上 Web typecheck 不通过：T-56 新增的 `apps/web/scripts/notifier-live-regression.mjs` 被 tsc 严格检查，报 14 处隐式 any

## 现象

2026-11-13 在 main 的内容上跑 `sh scripts/ci-local.sh`，其余步骤全过，`Web typecheck` 失败：

```
> react-router typegen && tsc --noEmit
scripts/notifier-live-regression.mjs(10,15): error TS7006: Parameter 'execFile' implicitly has an 'any' type.
scripts/notifier-live-regression.mjs(11,9): error TS7034: Variable 'cases' implicitly has type 'any[]' in some locations where its type cannot be determined.
…（共 14 处，全部在这一个文件）
```

这个文件是 T-56（`0a8adde` / `7f19a56`）的定位轮新增的复现脚本。T-56 合并时宿主门禁处于关闭状态，合并前只跑了复现命令，没有跑 typecheck，所以带进了 main。

## 复现步骤

1. `cd apps/web && { [ -d node_modules ] || npm ci; } && npm run typecheck`
2. 退出码非 0，输出如上。

## 期望行为

- `npm run typecheck`（即 ci-local 的 Web typecheck 步骤）通过；
- T-56 的复现命令 `cd apps/web && node --test scripts/notifier-live-regression.mjs` 仍然通过（2 条用例），脚本的断言内容不被削弱；
- `npm run lint`、`npm test`、`npm run build` 不受影响。

怎么修由修复轮定：给脚本补 JSDoc 类型，或把 `scripts/` 下的纯 Node 脚本排除出 `tsconfig` 的检查范围，取改动最小且不降低 `app/`、`server/`、`test/` 检查强度的那种。

## 实际行为

见现象。main 的 Web typecheck 是红的，恢复宿主门禁之前必须修好。

## 环境

main `b15e6d0` 之后；`apps/web/tsconfig.json`、`apps/web/scripts/notifier-live-regression.mjs`、`apps/web/package.json` 的 `typecheck` 脚本。Node 26.5.0。

## 线索（可选）

- **定位轮注意内核的回归测试规则（T-63 尚未修）**：定位轮必须新增或修改一个内核认得出的测试文件，否则修复轮做完会报「回归测试保护集为空」。认得出的只有：路径里有 `test/`、`tests/`、`__tests__/` 目录的文件，文件名带 `.test.` / `.spec.` / `_test.` 的文件，或者**以仓库相对路径原样出现在复现命令里**的脚本（`cd 子目录 && 相对路径` 认不出）。可行的写法之一：在 `apps/web/test/` 下新增一条用例守住「`tsc --noEmit` 对 scripts/ 下的脚本不报错」，复现命令跑这条用例；或者复现命令直接用仓库相对路径调用一个新增的检查脚本。
- 宿主门禁目前关闭（`.autome/config.toml` 的 `[gate]` 被注释），本任务的修复门禁只会跑复现命令；请修复轮自己跑一遍 `npm run typecheck && npm run lint && npm test` 并把结果写进 Bug 文档。
