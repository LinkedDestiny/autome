# M-03 r4 审计复验

- 验收命令（9 文件）：60 例全过，skipped 0，与预期 60 一致（本环境允许监听，w9-http 5 例取到证据，实现轮缺的 5 例已补）。
- 负向对照（改后跑 onboarding-watch、settings-onboarding、settings-onboarding-real、w9-http 四文件，改完全部还原，git status 干净）：
  1. 完成只 advance 一次 → settings-onboarding 第 2 条、settings-onboarding-real 第 1 条失败（2 failed）。
  2. onTick 不发布 onboarding.ended → onboarding-watch watch 用例、settings-onboarding-real 第 1 条失败（2 failed）。
  3. action 把 content 截断到 1000 字符 → settings-onboarding 3、4，settings-onboarding-real 超长保存，w9-http 超长保存共 4 条失败。
  还原后四文件 21 例全过。
- 判别结论：验收命令能拦住三类错误实现；产品行为符合设计。无人工项移出。
