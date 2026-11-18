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
it('C-2 时间线新增头部会话不让浏览器锚定改变外层滚动位置', () => {
  expect(rule('.task-timeline')).toMatch(/overflow-anchor:\s*none/);
  expect(rule('.task-timeline')).toMatch(/overflow-y:\s*auto/);
  expect(rule('.session-log-text')).toMatch(/overflow-anchor:\s*none/);
});
it('Layer 1 token 名值逐条一致', () => {
  expect(declarations(tokens)).toEqual({
    "--animal-font": "Nunito, 'Noto Sans SC', -apple-system, 'PingFang SC', 'Hiragino Sans GB', sans-serif",
    "--animal-primary": "#19c8b9",
    "--animal-primary-hover": "#3dd4c6",
    "--animal-primary-active": "#11a89b",
    "--animal-primary-bg": "#e6f9f6",
    "--animal-text": "#794f27",
    "--animal-text-body": "#725d42",
    "--animal-text-secondary": "#9f927d",
    "--animal-text-muted": "#8a7b66",
    "--animal-text-disabled": "#c4b89e",
    "--animal-bg": "#f8f8f0",
    "--animal-bg-content": "rgb(247, 243, 223)",
    "--animal-bg-secondary": "#f0e8d8",
    "--animal-bg-disabled": "#f0ece2",
    "--animal-bg-input": "#fffbe7",
    "--animal-border": "#c4b89e",
    "--animal-border-hover": "#a89878",
    "--animal-border-strong": "#9f927d",
    "--animal-border-light": "#e8dcc8",
    "--animal-radius-sm": "12px",
    "--animal-radius": "18px",
    "--animal-radius-lg": "24px",
    "--animal-radius-pill": "50px",
    "--animal-shadow-btn": "#bdaea0",
    "--animal-shadow-input": "#d4c9b4",
    "--animal-shadow-switch": "#5a9e1e",
    "--animal-shadow-sm": "0 2px 4px 0 rgba(61,52,40,.06)",
    "--animal-shadow-base": "0 3px 10px 0 rgba(61,52,40,.10)",
    "--animal-shadow-lg": "0 8px 24px 0 rgba(61,52,40,.14)",
    "--animal-focus-yellow": "#ffcc00",
    "--animal-focus-yellow-d": "#e0b800",
    "--animal-sidebar-active": "#b7c6e5",
    "--animal-sidebar-hover": "#d6dff0",
    "--animal-success": "#6fba2c",
    "--animal-success-active": "#5a9e1e",
    "--animal-warning": "#f5c31c",
    "--animal-warning-active": "#dba90e",
    "--animal-error": "#e05a5a",
    "--animal-error-active": "#c94444",
    "--animal-ease": "cubic-bezier(0.4, 0, 0.2, 1)",
    "--animal-duration-fast": ".15s",
    "--animal-duration": ".25s",
    "--animal-duration-slow": ".35s",
    "--tile-pink": "#f8a6b2",
    "--tile-purple": "#b77dee",
    "--tile-blue": "#889df0",
    "--tile-yellow": "#f7cd67",
    "--tile-orange": "#e59266",
    "--tile-teal": "#82d5bb",
    "--tile-green": "#8ac68a",
    "--tile-red": "#fc736d",
    "--tile-lime": "#d1da49",
    "--tile-brown": "#9a835a",
    "--tile-peach": "#e18c6f",
    "--ui-card-yellow-bg": "#fff8e0",
    "--ui-card-yellow-border": "#f7cd67",
    "--ui-card-yellow-text": "#8a6a10",
    "--ui-pat-yellow-1": "rgba(247,205,103,.22)",
    "--ui-pat-yellow-2": "rgba(255,225,150,.14)",
    "--ui-card-teal-bg": "#e8faf5",
    "--ui-card-teal-border": "#82d5bb",
    "--ui-card-teal-text": "#2e8a6e",
    "--ui-pat-teal-1": "rgba(130,213,187,.2)",
    "--ui-pat-teal-2": "rgba(170,235,210,.12)",
    "--ui-card-purple-bg": "#f0e8ff",
    "--ui-card-purple-border": "#b77dee",
    "--ui-card-purple-text": "#6f4aa8",
    "--ui-pat-purple-1": "rgba(183,125,238,.18)",
    "--ui-pat-purple-2": "rgba(210,180,255,.12)",
    "--ui-card-blue-bg": "#e8edff",
    "--ui-card-blue-border": "#889df0",
    "--ui-card-blue-text": "#4f5fb0",
    "--ui-pat-blue-1": "rgba(136,157,240,.18)",
    "--ui-pat-blue-2": "rgba(180,195,255,.12)",
    "--ui-card-green-bg": "#e8f5e8",
    "--ui-card-green-border": "#8ac68a",
    "--ui-card-green-text": "#3f7a3f",
    "--ui-pat-green-1": "rgba(138,198,138,.2)",
    "--ui-pat-green-2": "rgba(180,225,180,.12)",
    "--ui-card-red-bg": "#ffe8e8",
    "--ui-card-red-border": "#fc736d",
    "--ui-card-red-text": "#b0413a",
    "--ui-pat-red-1": "rgba(252,115,109,.18)",
    "--ui-pat-red-2": "rgba(255,180,175,.12)",
    "--ui-card-dash-bg": "rgb(250,248,242)",
    "--ui-card-border": "#d4c4a8",
    "--ui-surface": "#fffdf3",
    "--ui-white": "#fff",
    "--ui-pill-teal": "#19c8b9",
    "--ui-pill-blue": "#889df0",
    "--ui-pill-purple": "#b77dee",
    "--ui-pill-orange": "#e59266",
    "--ui-pill-pink": "#f08a9a",
    "--ui-pill-brown": "#9a835a",
    "--ui-pill-green": "#6fba2c",
    "--ui-pill-yellow": "#f5c31c",
    "--ui-pill-red": "#e05a5a",
    "--ui-ribbon-teal-front": "#19c8b9",
    "--ui-ribbon-teal-back": "#11a89b",
    "--ui-ribbon-yellow-front": "#f7cd67",
    "--ui-ribbon-yellow-back": "#d4a030",
    "--ui-ribbon-blue-front": "#889df0",
    "--ui-ribbon-blue-back": "#5f78d8",
    "--ui-ribbon-green-front": "#8ac68a",
    "--ui-ribbon-green-back": "#5fa05f",
    "--ui-ribbon-brown-front": "#9a835a",
    "--ui-ribbon-brown-back": "#7a6440",
    "--ui-row-gate": "#e8f5e8",
    "--ui-row-you": "#fff8e0",
    "--ui-shadow-input": "inset 0 2px 0 rgba(212,201,180,.35)",
    "--ui-shadow-progress": "inset 0 1px 3px rgba(61,52,40,.12)"
  });
});
it('字体和许可完整复制，字体仅从本地加载', () => {
  const hashes: Record<string, string> = {
  "Nunito-Variable.ttf": "bb55a5ca5c2042335b3991af27c4d0705d0ef41cac6164ac737fd8f2a1e85207",
  "OFL.txt": "580df76c95a1ec5ab878ceb25bb3d85c6a076804e9c970c8c6972aea775fdf65"
};
  for (const name of ['Nunito-Variable.ttf', 'OFL.txt']) {
    const hash = (buffer: Buffer) => createHash('sha256').update(buffer).digest('hex');
    expect(hash(readFileSync(new URL(`styles/fonts/nunito/${name}`, app)))).toBe(hashes[name]);
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
it('看板高度受限且四列各自滚动，页面不增加滚动容器', () => {
  expect(rule('.board')).toContain('min-height: 0'); expect(rule('.board')).toContain('height: 100%');
  expect(rule('.board-columns')).toContain('min-height: 0'); expect(rule('.board-columns')).toContain('repeat(4, minmax(0, 1fr))');
  expect(rule('.board-column')).toContain('min-height: 0');
  expect(rule('.board-column-body')).toContain('overflow-y: auto'); expect(rule('.board-column-body')).toContain('min-height: 0');
  for (const r of rules.filter(r => /^(html|body)$/.test(r.selector))) expect(r.body).not.toMatch(/overflow(?:-y)?:\s*(auto|scroll)/);
});
it('新建页正文和附件内部滚动', () => {
  expect(rule('.task-new')).toContain('height: 100%'); expect(rule('.task-new')).toContain('overflow: hidden');
  expect(rule('.task-new textarea')).toContain('min-height: 0'); expect(rule('.task-new textarea')).toContain('overflow-y: auto');
  expect(rule('.task-new-attachments')).toContain('overflow-y: auto');
});

it('app.css 的每个 var 都有令牌定义', () => {
  const defined = declarations(tokens);
  for (const [, name] of css.matchAll(/var\((--[\w-]+)/g)) expect(defined, name).toHaveProperty(name);
});
it('每个 ui 令牌都被 app.css 引用', () => {
  for (const name of Object.keys(declarations(tokens)).filter(name => name.startsWith('--ui-'))) expect(css, name).toContain('var(' + name + ')');
});
it('SVG 只出现在图标与进度条组件', () => {
  const owners = files.filter(file => file.endsWith('.tsx') && /<svg\b/.test(read(file))).sort();
  expect(owners).toEqual(['components/icons.tsx', 'components/ui/progress.tsx']);
});
