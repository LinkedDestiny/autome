export type TaskKind = 'feature' | 'bug';
export const TASK_TEMPLATES: Record<TaskKind, string> = {
  feature: '# <标题>\n## 背景\n<!-- 为什么要做 -->\n## 要做什么\n<!-- 具体行为，讨论中定下的细节逐条写 -->\n## 不做什么\n<!-- 明确排除的范围 -->\n## 验收标准\n<!-- 可检查的条目 -->\n## 参考\n<!-- 仓库内文件、链接、截图（附件） -->\n',
  bug: '# <标题>\n## 现象\n<!-- 看到了什么 -->\n## 复现步骤\n<!-- 1. … 2. … 3. … -->\n## 期望行为\n## 实际行为\n<!-- 报错原文、日志片段 -->\n## 环境\n<!-- 版本、分支、配置 -->\n## 线索（可选）\n<!-- 怀疑的位置、最近的相关改动 -->\n',
};
export function switchKind(body: string, from: TaskKind, to: TaskKind) {
  return body === TASK_TEMPLATES[from] || !body.trim()
    ? { body: TASK_TEMPLATES[to], kept: false } : { body, kept: true };
}
