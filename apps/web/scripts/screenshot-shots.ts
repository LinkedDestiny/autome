import { TEST_IDS } from '../app/lib/testids.ts';

export type Shot = {
  name: string;
  path: string;
  click?: { testId: typeof TEST_IDS[keyof typeof TEST_IDS]; button?: boolean };
  scroll?: string;
};

export function parseScreenshotArgs(args: string[]): { set: 'readme' | 'all'; out: string } {
  let set: 'readme' | 'all' = 'readme';
  let out = 'docs/images';
  const seen = new Set<string>();
  for (let i = 0; i < args.length; i += 2) {
    const key = args[i];
    const value = args[i + 1];
    if (!['--set', '--out'].includes(key) || seen.has(key) || !value?.trim() || value.startsWith('--')) {
      throw Error('用法：screenshots [--set readme|all] [--out <目录>]');
    }
    seen.add(key);
    if (key === '--set') {
      if (value !== 'readme' && value !== 'all') throw Error(`未知截图集合：${value}`);
      set = value;
    } else out = value;
  }
  // The comparison set must never overwrite the README assets by default.
  if (set === 'all' && !seen.has('--out')) throw Error('all 集合必须指定 --out <目录>');
  return { set, out };
}

export function screenshotShots(set: 'readme' | 'all', projectId: string, taskId: string): Shot[] {
  const project = encodeURIComponent(projectId);
  const common: Shot[] = [
    { name: 'project', path: `/projects/${project}` },
    { name: 'task', path: `/tasks/${encodeURIComponent(taskId)}` },
    { name: 'loop', path: '/loop' },
    { name: 'skills', path: '/skills' },
  ];
  if (set === 'readme') return [{ name: 'dashboard', path: '/' }, ...common];
  return [...common,
    { name: 'loop-bottom', path: `/loop?project=${project}`, scroll: '.loop-defaults' },
    { name: 'loop-drawer', path: '/loop', click: { testId: TEST_IDS.loopRole, button: true } },
    { name: 'loop-projinfo', path: `/loop?project=${project}&tab=project` },
    { name: 'skills-click', path: '/skills', click: { testId: TEST_IDS.skillCard } },
    { name: 'project-new', path: '/projects/new' },
    { name: 'env', path: '/env' },
  ];
}
