import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

export function writeWindowScripts(root: string, slug: string) {
  const fake = join(root, 'repo/.autome/fake', slug);
  mkdirSync(fake, { recursive: true });
  writeFileSync(join(fake, 'next'), '1');
  for (let step = 1; step <= 23; step++) {
    const round = step === 1 ? 0 : Math.floor((step + 2) / 3);
    const document = `# 窗口设计正文\n\nstatus: 设计中\ndesign-round: ${round}/15\nimplementation-round: 0/25\ncurrent-milestone: M-01\ncurrent-milestone-reopens: 0\nconvergence-mode: normal\nnext-action: 继续评审（第 ${step} 步）\n\n## 里程碑\n\n| ID | 状态 | 标题 | reopen | 领域 |\n|---|---|---|---|---|\n| M-01 | 开放 | 窗口验证 | 0 | |\n\n## Backlog\n\n## 争议项\n`;
    writeFileSync(join(fake, `${step}.sh`), `set -e
mkdir -p docs/${slug}
cat > docs/${slug}/${slug}.md <<'DOC'
${document}DOC
i=0
while [ "$i" -lt 400 ]; do echo "window evidence line $i"; i=$((i+1)); done
${step >= 22 ? `while [ ! -f '${root}/release-${slug}-${step}' ]; do '${root}/bin/w7-sleep' 0.1; done` : ''}
git add docs/${slug}
git -c user.name=fake -c user.email=f@f commit -qm 'window ${step}'
`);
  }
  return fake;
}
