# M-11 · 实现 #13 · 开放（环境阻塞）

## 改动与结论

README 更新为五图英文说明；截图脚本增加全局 Loop 配置、技能页、三个隔离技能，复用真实协议夹具生成等待批准任务及保持运行的任务，并断言总览黄/青卡片。夹具预算对齐现有 PREPARE_TIMEOUT_MS；相对 --out 路径统一从仓库根解析，确保交接产物进入根目录 .autome/output。图片测试改为恰好五图并新增 Loop/技能非空检查。

未生成或替换图片，不把旧图当新图。M-11 保持开放，未标待审，未增加 reopen。上一轮审计 M-10 通过，没有应逐字迁入的缺陷检查。

## 命令与结果

- `npm run build --prefix apps/web`：通过；原始输出 `.autome/output/m11-build.txt`。
- `npm run screenshots --prefix apps/web`：失败，`spawnSync ps EPERM`，在进程清理能力预检即停止，尚未启动夹具/监听端口/浏览器；输出 `.autome/output/m11-screenshots.txt`。没有绕过清理预检。
- 契约命令 `(cd apps/web && npx vitest run test/readme-images.test.ts test/docs-web.test.ts) && (cd apps/web && npx playwright test e2e/layout.spec.ts)`：Vitest 6 个，3 过、3 败，因 docs/images 仍仅旧三图且 loop.png/skills.png 缺失，后半段短路。Vitest 输出 `.autome/output/m11-acceptance.txt`。
- 单独执行后半段 `(cd apps/web && npx playwright test e2e/layout.spec.ts)`：globalSetup 同样 `ps EPERM`；14 个布局用例均未执行，不能算通过或正常跳过。输出 `.autome/output/m11-layout.txt`。
- `sh scripts/ci-quick.sh --since 054ebf0f533ca0dedf6afd74e394da0f843c38a7`：9 步执行，1 步失败（图片测试），静态检查通过；输出 `.autome/output/m11-quick.txt`。没有运行 ci-local 或全量门禁。
- `git diff --check`：通过。

用例总数按本轮验收选集为 20（6 Vitest + 14 Playwright），相较起点相同选集 19 增加 1，来自新增「Loop 配置与技能截图存在且非空」。实际执行 6，未执行 14。上一轮 M-10 为 107 Vitest + 5 浏览器 = 112，本轮选集比它少 92，是里程碑验收范围切换，不是删除或跳过原测试。快检选入的 readme-images 4 个是上述 6 个的子集，不重复计数。

## 标待审之前的自审清单（本轮未达到标待审条件）

1. **情形表逐行**：README 五图引用已更新；目录恰好五图、五张 1512×944 PNG、两张新图非空均由测试检出未满足；截图脚本说明测试通过。总览等待/运行卡、项目、任务、全局 Loop、技能五个截图目标已写入脚本，但运行被预检阻断，不能证明画面正确。布局 14 项（技能、Loop、总览、命令面板、铃铛、项目、全部已完成、新建任务、需求详情展开日志、Bug 详情、添加项目补全、项目设置、环境、已取消展开）全部未取到运行证据。
2. **失败分支**：G-Q3 的环境阻塞交接分支已实际触发；根因比监听端口更早，为 ps 不可用。缺图负向对照实际为红（3 个失败），不伪造 PNG 补齐。授权/读盘/外部服务业务失败不适用：本轮仅截图资产与文档，不变更产品错误处理。
3. **全部迁移**：不适用，本里程碑不增加产品状态机；夹具等待批准与运行状态只是截图前置，新增断言仍待非受限环境执行。
4. **输入域边界**：图片数量 5、尺寸 1512×944、PNG 文件头和非空均有断言，因缺图验收未通过；没有新增用户业务输入域。--out 保留原有参数校验并统一相对路径锚点。

## 下一轮交接

在允许进程枚举与监听端口的环境运行 `npm run screenshots --prefix apps/web -- --out .autome/output/screens-M11`，检查五张实际画面后复制到 docs/images；随后运行完整 M-11 验收命令及指定快检。未验证脚本新增浏览器断言、未进行视觉通过判断。无需用户授权或把环境问题写成协议失败。
