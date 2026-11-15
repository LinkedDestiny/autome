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

export const SETTINGS = {
  config: '配置（只读）', unset: '未配置', role: '角色', enabled: '已启用', disabled: '已禁用',
  remove: '移除项目', warning: '移除只是取消注册，不会删除任何文件；目录原样保留，之后可以重新添加。',
  name: '输入项目名以确认移除', mismatch: '已拒绝：输入的项目名不一致',
};
export const CONFIG_LABELS: Record<string, string> = {
  gate_quick_concurrency: '快检并行数', 'setup.command': '初始化命令', 'setup.timeout_minutes': '初始化超时（分钟）', 'merge_after.command': '合并后命令', 'merge_after.timeout_minutes': '合并后超时（分钟）', gate: '门禁命令', gate_quick: '快检命令',
  'bug.fix_rounds': '修复轮数', 'gate.command': '门禁命令', 'gate.timeout_minutes': '门禁超时（分钟）', gate_run: '门禁运行时机', 'gate_quick.command': '快检命令', 'gate_quick.timeout_minutes': '快检超时（分钟）', setup: '初始化命令',
  gate_concurrency: '门禁并行数', gate_reuse_for_main_check: '复用主分支检查', gate_same_failure_limit: '相同失败上限',
  'loop_defaults.parallel': '并行数', 'loop_defaults.session_minutes': '会话时长上限（分钟）',
  'loop_defaults.budget_factor': '预算系数', 'loop_defaults.design_rounds': '设计轮数',
  merge_after: '合并后命令', merge_auto: '自动合并', 'retry.attempts': '重试次数', 'retry.backoff_seconds': '重试等待（秒）',
};

export const ALERTS = { title: '项目报警', dismiss: '知道了', readError: '报警读取失败：', count: (n: number) => `报警 ${n}` };
export const ALERT_KINDS: Record<string, string> = {
  main_gate_failed: '主分支门禁失败', setup_failed: '初始化失败',
  after_merge_failed: '合并后命令失败', budget_exceeded: '预算超限',
};

export const ENV = {
  drivers: '驱动', selfDriver: '本面板', driving: '推进中', stoppedDriving: '已停止推进',
  driverStartedAt: '启动时间', driverLastTickAt: '最近一次 tick',
  title: '环境检查', detect: '重新检测', probing: '正在检测…', installed: '已安装', missing: '未安装',
  noLogin: '不需要登录', loggedIn: '已登录', expired: '登录已过期', unknown: '登录状态未知',
  attention: '需要处理', version: '版本', path: '路径', prerequisite: '前置条件', none: '无', met: '已满足', unmet: '未满足',
  install: '在终端里安装', login: '在终端里登录', copy: '复制', copied: '已复制', copyError: '复制失败，请手动复制',
  launched: '已在终端里执行', fallback: '请复制下面的命令，自己在终端里执行', terminalError: '打不开终端：',
  go: '去环境页', summary: (n: number) => `本机环境有 ${n} 项需要处理：`,
};
export const COMPONENT_NAMES: Record<string, string> = { git: 'Git', claude: 'Claude Code', codex: 'Codex', iterm2: 'iTerm2' };
