export const ADMIN = {
  add: '添加项目', environment: '环境', path: '项目路径', check: '检查', checking: '处理中…',
  workspace: '按工作区添加', docs: '文档仓库', skip: '跳过 Onboarding', confirm: '确认添加',
  picker: '选择目录…', more: '还有更多，请继续输入', project: '查看项目', settings: '项目设置',
  offline: '内核未连接：服务未接入网关', unavailable: '路径校验服务不可用', unknown: '未知操作',
  partial: '项目已添加；跳过 Onboarding 时内核报错：', completionError: '目录补全失败：',
  missing: '目录不存在。确认后会创建它，并在里面 git init（默认分支 main）',
  plain: '不是 Git 仓库。确认后会在这里 git init（默认分支 main）',
  repo: '已是 Git 仓库，直接接管', suggested: '看起来是工作区',
  nested: '按单仓库添加会在外层目录 git init，里面的仓库会变成嵌套仓库',
};

export const ONBOARDING = {
  title: 'Onboarding', missing: '项目不存在', run: '运行', save: '保存', finish: '完成', skip: '跳过',
  ended: '已结束', skipped: '已跳过', none: '没有会话', running: '会话运行中', interrupted: '会话已中断',
  invalidPath: '已拒绝：只能保存项目画像或 AGENTS.md', noCommit: '没有需要提交的改动',
  saved: '已保存', unfinished: 'Onboarding 尚未结束，请重试',
  step: (n: number) => `第 ${n} / 5 步`,
  descriptions: { 3: '起草项目画像与 AGENTS.md', 4: '确认或编辑', 5: '查看 Loop 配置' } as Record<number, string>,
  count: (n: number, max: number) => `${n} / ${max} 字符`,
  exit: (code?: number) => `会话已结束，退出码 ${code ?? '未知'}`,
  commit: (sha: string) => `已生成 init commit ${sha.slice(0, 8)}`,
};
