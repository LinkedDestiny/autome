# M-03 · 实现 #4

结论：验收通过，交独立审计；implementation-round 为 4/25。上一轮 M-02 审计通过，无退回缺陷，也没有需要逐字迁入的审计判别检查。本轮仅推进 M-03。

## 实现与修复

- 简报增加可选集成段，显示四种触发原因、逐仓库范围、列出/消失提交、路径、stat、截断计数、测试清单与确切证据路径。无段时旧输出逐字不变。
- 调度器把已选定集成身份携带的 range 同时用于简报与 prompt；重试复用首次审计的事实。工作区测试清单沿用账本中已有的仓库前缀，不重复添加。
- 独占行取舍函数拒绝不配对、嵌套和非独占标记；普通审计删除整个块，集成审计保留正文。新增四份金样，既有金样无修改/删除。
- case.yaml 增加默认 false 的 integration_audit；评估运行器先取舍再渲染，原 render_prompt 签名不变。发布 v7 J-1，新增 eval scaffold/grader 并嵌入协议种子。
- 直接相关回归发现嵌入清单遗漏及发布版本常量未同步：第一次 protocol:: 为 61 通过/9 失败；补入三份 eval 文件，将精确版本期望 v6 同步为设计明确要求的 v7，复跑 70/70。设计 §11 已列出此必要配套，不改行为断言或测试前提，哈希断言保留。
- scaffold 冒烟首次因 shell 将中文标点连入 `$end` 变量名失败；改成 `${end}` 后，在独立临时 Git 仓库完整生成通过。

## 命令与用例数量

本轮里程碑验收原命令：

`cargo test -p automed --test prompt_rendering && cargo test -p automed --lib integration_section && cargo run -p automed -- protocol eval protocol/ && git diff --quiet --diff-filter=MD "$(git merge-base HEAD main)" -- crates/automed/tests/golden/`

结果全为退出码 0。prompt_rendering **35** 例，相比上一轮/设计基线 31 **+4**，即四份集成 prompt/brief 金样；integration_section **6** 例，相比基线 0 **+6**（brief 3、launcher 2、eval 1）。验收测试合计 **41**，基线 31，**+10**，无失败/忽略。协议静态评估 **8 项通过、0 项失败、1 项警告**：43 行历史规范没有 CHANGELOG 依据；这不是行为评估。原始输出：`.autome/output/m03-final-{prompts,sections,eval}.txt`。

直接相关回归：

- `cargo test -p automed --lib integration_wiring`：5/5，基线 5，差 0。
- `cargo test -p automed --lib protocol::`：70/70，基线 69，+1（eval 标记取舍用例）。
- `cargo test -p automed --lib launcher::`：78/78，基线 76，+2（标记选择及错误域）。
- `cargo test -p automed --lib brief::`：28/28，基线 25，+3（简报集成段）。

以上与 integration_section 的 6 例重叠，去重后本轮项目测试 **216** 例，对应相同筛选基线 **206**，净增 **10**；不是全项目用例总数。日志在 `.autome/output/m03-{wiring,protocol-green,launcher,brief}.txt`。

另在 `.autome/output/m03-eval-fixture-green/` 的独立 Git 仓库运行 scaffold，并执行 `rustc --test tests/integration.rs -o integration-test && ./integration-test`：fixture **1** 例通过，基线 0，+1。这只是评估夹具可执行性，不计入上述 216，也不冒充真实 Agent 行为评估。日志 `.autome/output/m03-scaffold-green.txt`。

未运行全量 ci-local、宿主快检或付费 `--behaviour`；依用户要求交宿主/后续流程。

## 标待审之前的自审清单

1. **情形表逐行**：§5 的 rebase 冲突/门禁失败分别由新金样覆盖；用户表态/无注入由 brief 单测覆盖。单仓库由两份 brief 金样覆盖，工作区由带 api 前缀单测覆盖且断言不存在 api/api/。列出、消失、stat 读取失败文本、两类省略计数均有断言；门禁段不带失败提交 SHA 或“未通过”。§6 去掉/保留两模式由单测和普通/集成金样覆盖；§4 版本门及后续实现使旧范围失效由 integration_wiring 5 例回归覆盖。
2. **失败分支**：不配对（缺开/缺闭）、嵌套、前后夹文字/前置空白的标记，在 keep true/false 下均返回错误；case 非布尔值拒绝；stat 读取失败提示仍展示。拒绝授权、超时、外部服务不可达不适用：本轮新增渲染为纯函数，无外部调用；范围读盘失败属于已完成 M-01，本轮不改其语义。
3. **全部迁移**：本轮无新增任务状态机。覆盖输入 integration None→普通简报/普通 prompt，Some→集成简报/集成 prompt；门禁 each_round/before_merge 两方向各有金样。eval 默认 false/显式 false/true 都验证。既有范围选择与重试基线由 integration_wiring 回归，不修改任务迁移。
4. **域边界**：空模板、无末尾换行、单行模板原样保留；空 repos、轮次 0、精确证据路径、单仓库/工作区、多提交及消失提交、超过展示数的省略提示覆盖；长 SHA 截为 7 字符并检查没有 10 字符前缀。日期/闰日/前导零不适用：没有日期解析或数值字符串输入域。

## 负向对照

全部注入已还原，最后重跑验收为绿。

- 去掉模式故意多留一空行：prompt_rendering 退出 101，既有 audit 金样断言失败。日志 `.autome/output/m03-negative-gap.txt`。
- integration None 仍输出段标题：`before_merge_audit_brief_matches_its_golden` 退出 101。日志 `.autome/output/m03-negative-none.txt`。
- SHA 截为 10 字符（模拟 Git short 扩长）：`integration_section_contains_facts` 退出 101。日志 `.autome/output/m03-negative-sha.txt`。

## 限制

协议正文静态检查已通过，但当前接近 24 KiB 上限；M-05 加正文时须继续遵守原预算，不扩大上限。真实 Agent 的 eval 红/绿未运行，不能由静态评估或夹具冒烟推定。
