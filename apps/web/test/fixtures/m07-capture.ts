import { writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { w9Kernel } from './w9-replies.ts';
const s = await w9Kernel();
try {
  const dir = join(s.userHome, '.claude/skills/m07-skill'); mkdirSync(dir, { recursive: true }); writeFileSync(join(dir, 'SKILL.md'), '# Skill');
  const { project } = await s.core.call('project.add', { path: s.repo('m07') }) as { project: { id: string } };
  const global = await s.core.call('config.get', {});
  await s.core.call('config.set_role', { project_id: project.id, role: 'plan', model: 'project-model' });
  for (const [name, value] of Object.entries({ global, project: await s.core.call('config.get', { project_id: project.id }), skills: await s.core.call('skills.list', {}) })) writeFileSync(new URL(`./m07-${name}.json`, import.meta.url), JSON.stringify(value, null, 2) + '\n');
} finally { await s.close(); }
