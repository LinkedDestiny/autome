// Browser selection shares the quick stage's revision and changed-path domain.
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { changedPaths } from './quick.mjs';

export function browserSelection() {
  const paths = changedPaths();
  if (!paths) return { reason: '未提供本轮起点，未跑浏览器用例' };
  const full = reason => ({ full: true, reason });
  const harness = 'apps/web/e2e/harness/';
  const fixtures = new Set();
  if (existsSync(harness)) for (const entry of readdirSync(harness, { recursive: true })) {
    if (!/\.[cm]?[jt]s$/.test(entry)) continue;
    for (const match of readFileSync(harness + entry, 'utf8').matchAll(/test\/fixtures\/([\w.-]+)/g)) fixtures.add(`apps/web/test/fixtures/${match[1]}`);
  }
  if (paths.some(p => p.startsWith(harness) || fixtures.has(p) || [
    'apps/web/playwright.config.ts', 'apps/web/scripts/e2e-preflight.ts',
    'apps/web/package.json', 'apps/web/package-lock.json',
  ].includes(p))) return full('浏览器共享输入变化');
  const specs = paths.filter(p => /^apps\/web\/e2e\/[^/]+\.spec\.ts$/.test(p) && existsSync(p)).map(p => p.slice(9));
  if (paths.some(p => /^apps\/web\/(app|server)\//.test(p))) {
    const evidence = paths.filter(p => /^docs\/[^/]+\/evidence\/M-\d+-r\d+-impl\.md$/.test(p));
    if (!evidence.length) return full('未定位本轮里程碑证据');
    for (const path of evidence) {
      const [, slug, milestone] = /^docs\/([^/]+)\/evidence\/(M-\d+)-/.exec(path);
      const design = `docs/${slug}/${slug}.md`;
      if (!existsSync(design)) return full('未定位里程碑验收契约');
      const section = readFileSync(design, 'utf8').split(`### ${milestone} 验收契约`)[1]?.split(/\n##?#? /)[0];
      const command = section?.split('\n').find(line => line.includes('验收命令'));
      const named = [...(command ?? '').matchAll(/e2e\/[\w.-]+\.spec\.ts/g)].map(m => m[0]);
      if (!named.length || named.some(p => !existsSync(`apps/web/${p}`))) return full('验收命令未点名现存 spec');
      specs.push(...named);
    }
  }
  return specs.length ? { specs: [...new Set(specs)].sort(), reason: '改动 spec 与里程碑验收命令' }
    : { reason: '本轮改动不涉及浏览器用例的输入' };
}
