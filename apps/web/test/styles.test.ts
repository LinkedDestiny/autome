import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { expect, it } from 'vitest';
const app = new URL('../app/', import.meta.url);
const read = (path: string) => readFileSync(new URL(path, app), 'utf8');
const tokens = read('styles/tokens.css');
const css = read('styles/app.css');
const files = readdirSync(app, { recursive: true }).map(String).filter(path => /\.tsx?$/.test(path));
const tsx = files.filter(path => path.endsWith('.tsx')).map(read).join('\n');
const rules = [...css.matchAll(/([^{}]+)\{([^}]+)\}/g)].map(([, selector, body]) => ({ selector: selector.trim(), body }));
function declarations(source: string) { return Object.fromEntries([...source.match(/:root\s*\{([^}]+)\}/)![1].matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map(([, k, v]) => [k, v.trim()])); }
function rule(selector: string) { const found = rules.find(r => r.selector === selector); expect(found, selector).toBeDefined(); return found!.body; }
it('Layer 1 token 名值逐条一致', () => {
  expect(declarations(tokens)).toEqual(declarations(readFileSync(new URL('../../desktop/renderer/style.css', import.meta.url), 'utf8')));
});
it('字体和许可完整复制，字体仅从本地加载', () => {
  const source = new URL('../../desktop/renderer/fonts/nunito/', import.meta.url);
  for (const name of ['Nunito-Variable.ttf', 'OFL.txt']) {
    const hash = (buffer: Buffer) => createHash('sha256').update(buffer).digest('hex');
    expect(hash(readFileSync(new URL(`styles/fonts/nunito/${name}`, app)))).toBe(hash(readFileSync(new URL(name, source))));
  }
  const path = tokens.match(/url\(['"]?([^'")]+)['"]?\)/)![1];
  expect(path.startsWith('./')).toBe(true); expect(existsSync(new URL(path, new URL('styles/', app)))).toBe(true);
  expect(tokens + css).not.toMatch(/url\(\s*['"]?(?:https?:|\/\/)/);
});
it('两个样式文件标明来源与许可', () => { for (const file of [tokens, css]) { expect(file.split('*/')[0]).toContain('animal-island-ui'); expect(file.split('*/')[0]).toContain('CC BY-NC 4.0'); } });
it('样式与 TSX 不散写颜色', () => { expect(css + tsx).not.toMatch(/#[\da-f]{3,8}\b|\brgba?\s*\(|\bhsla?\s*\(|\bblack\b/i); });
it('全部焦点环都是黄色', () => { const focus = rules.filter(r => r.selector.includes(':focus-visible')); expect(focus.length).toBeGreaterThan(0); for (const r of focus) expect(r.body).toMatch(/outline:\s*3px solid var\(--animal-focus-yellow\)/); });
it('圆角使用约定 token 或圆形', () => { for (const [, radius] of css.matchAll(/border-radius:\s*([^;]+);/g)) expect(radius).toMatch(/^(var\(--animal-radius[\w-]*\)|50%)$/); });
it('3D 阴影只给主按钮', () => { const shadow = rules.filter(r => r.body.includes('--animal-shadow-btn')); expect(shadow).toHaveLength(1); expect(shadow[0].selector).toBe('.btn-primary'); });
it('源码没有 emoji 图标', () => { for (const file of files) expect(read(file), file).not.toMatch(/\p{Extended_Pictographic}/u); });
it('TSX 没有内联样式', () => { expect(tsx).not.toMatch(/style=\{/); });
it('外壳约束页面高度，四个列表内部滚动', () => {
  expect(rule('.shell')).toContain('height: 100dvh'); expect(rule('.shell')).toContain('overflow: hidden');
  expect(rule('.shell-main')).toContain('min-height: 0'); expect(rule('.shell-main')).toContain('overflow: hidden');
  for (const name of ['.project-list', '.overview-column-body', '.palette-list', '.bell-panel']) { expect(rule(name)).toContain('overflow-y: auto'); expect(rule(name)).toContain('min-height: 0'); }
  expect(rule('body')).toContain('margin: 0');
  for (const r of rules.filter(r => /^(html|body)$/.test(r.selector))) expect(r.body).not.toMatch(/overflow(?:-y)?:\s*(auto|scroll)/);
});
