'use strict';

// Static checks over the renderer's source, under plain `node --test`.
//
// These are the invariants that a DOM test cannot see, because a violation of
// them is a call that happens to be on a code path no fixture reached: a
// screen calling `read('taskChanged')` because someone renamed the preload
// function, or an `innerHTML` assignment in the one branch that renders a
// milestone title. Both are found by reading the files, and only by reading
// the files.

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const RENDERER = path.join(__dirname, '..', 'renderer');
const PRELOAD = path.join(__dirname, '..', 'preload.js');

function sourceFiles() {
  const out = [];
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === 'fonts') continue;
        walk(full);
      } else if (entry.name.endsWith('.js')) {
        out.push(full);
      }
    }
  };
  walk(RENDERER);
  return out;
}

/**
 * The names the preload actually puts on `window.autome.read` /
 * `window.autome.write`. Parsed from the file rather than listed here, so the
 * two cannot drift: if the preload drops a function, this test starts failing
 * for the screens that still call it.
 */
function preloadSurface() {
  const source = fs.readFileSync(PRELOAD, 'utf8');
  const section = (name) => {
    const start = source.indexOf(`  ${name}: {`);
    assert.ok(start >= 0, `preload.js has no \`${name}\` section`);
    // Walk braces from the section's opening one so nested object literals
    // (setRole builds one) do not end the section early.
    let depth = 0;
    let i = source.indexOf('{', start);
    const from = i;
    for (; i < source.length; i += 1) {
      if (source[i] === '{') depth += 1;
      else if (source[i] === '}') {
        depth -= 1;
        if (depth === 0) break;
      }
    }
    const body = source.slice(from + 1, i);
    // Top-level `name:` keys only — indented exactly four spaces.
    return new Set(Array.from(body.matchAll(/^ {4}([A-Za-z_$][\w$]*):/gm), (m) => m[1]));
  };
  return { read: section('read'), write: section('write') };
}

test('preload exposes named functions only, never a generic invoke', () => {
  const source = fs.readFileSync(PRELOAD, 'utf8');
  assert.ok(!/\binvoke\s*:/.test(source), 'preload must not expose a generic `invoke`');
  const surface = preloadSurface();
  assert.ok(surface.read.size >= 8, `read surface looks wrong: ${[...surface.read]}`);
  assert.ok(surface.write.size >= 20, `write surface looks wrong: ${[...surface.write]}`);
});

test('every read the renderer performs is a function the preload exposes', () => {
  const surface = preloadSurface();
  const missing = [];
  for (const file of sourceFiles()) {
    const source = fs.readFileSync(file, 'utf8');
    // lib/api.js is the only module allowed to name `window.autome`; the
    // screens go through `read('name', ...)` / `readOr(fallback, 'name', ...)`.
    for (const match of source.matchAll(/\bread\(\s*'([A-Za-z_$][\w$]*)'/g)) {
      if (!surface.read.has(match[1])) missing.push(`${path.basename(file)}: read.${match[1]}`);
    }
    for (const match of source.matchAll(/\breadOr\(\s*[^,]+,\s*'([A-Za-z_$][\w$]*)'/g)) {
      if (!surface.read.has(match[1])) missing.push(`${path.basename(file)}: readOr ${match[1]}`);
    }
  }
  assert.deepEqual(missing, [], `renderer names reads the preload does not expose: ${missing.join(', ')}`);
});

test('every write the renderer performs is a function the preload exposes', () => {
  const surface = preloadSurface();
  const missing = [];
  for (const file of sourceFiles()) {
    const source = fs.readFileSync(file, 'utf8');
    // `attempt({ run: (write) => write.foo(...) })` is the only shape.
    for (const match of source.matchAll(/\bwrite\.([A-Za-z_$][\w$]*)\s*\(/g)) {
      if (!surface.write.has(match[1])) missing.push(`${path.basename(file)}: write.${match[1]}`);
    }
  }
  assert.deepEqual(missing, [], `renderer names writes the preload does not expose: ${missing.join(', ')}`);
});

test('the renderer covers the reads it needs: every screen reads something', () => {
  // Counted from the router's own table rather than written down here: a
  // hard-coded number turns "a screen was added" into a failure of this test
  // instead of a failure of whatever the new screen got wrong.
  const routed = fs.readFileSync(path.join(__dirname, '..', 'renderer/app.js'), 'utf8');
  const expected = new Set(
    Array.from(routed.matchAll(/from '\.\/screens\/(\w+)\.js'/g)).map((m) => m[1])
  ).size;
  const screens = sourceFiles().filter((f) => f.includes(`${path.sep}screens${path.sep}`));
  assert.equal(
    screens.length,
    expected,
    `the router imports ${expected} screens but ${screens.length} exist`
  );
  for (const file of screens) {
    const source = fs.readFileSync(file, 'utf8');
    assert.match(
      source,
      /export async function load\(/,
      `${path.basename(file)} must export an async load()`
    );
    assert.match(
      source,
      /export function render\(/,
      `${path.basename(file)} must export render()`
    );
    assert.match(source, /export const id =/, `${path.basename(file)} must export an id`);
    assert.match(source, /export const nav =/, `${path.basename(file)} must export a nav key`);
  }
});

test('no renderer module assigns innerHTML or outerHTML', () => {
  const offenders = [];
  for (const file of sourceFiles()) {
    const source = fs.readFileSync(file, 'utf8');
    if (/\.(inner|outer)HTML\s*=/.test(source)) offenders.push(path.basename(file));
    if (/insertAdjacentHTML/.test(source)) offenders.push(`${path.basename(file)} (insertAdjacentHTML)`);
  }
  assert.deepEqual(offenders, [], `markup must never be built from data: ${offenders.join(', ')}`);
});

test('only lib/api.js reaches for window.autome', () => {
  const offenders = [];
  for (const file of sourceFiles()) {
    if (path.basename(file) === 'api.js') continue;
    const source = fs.readFileSync(file, 'utf8');
    if (/window\.autome\b/.test(source)) offenders.push(path.basename(file));
  }
  assert.deepEqual(offenders, [], `the IPC bridge has exactly one caller: ${offenders.join(', ')}`);
});

test('no renderer module sets an inline style attribute (the CSP forbids it)', () => {
  const offenders = [];
  for (const file of sourceFiles()) {
    const source = fs.readFileSync(file, 'utf8');
    if (/setAttribute\(\s*'style'/.test(source)) offenders.push(path.basename(file));
  }
  assert.deepEqual(offenders, [], `use el.style.setProperty instead: ${offenders.join(', ')}`);
});

test('index.html declares the CSP and loads app.js as a module', () => {
  const html = fs.readFileSync(path.join(RENDERER, 'index.html'), 'utf8');
  assert.match(html, /Content-Security-Policy/);
  assert.match(html, /default-src 'none'/);
  assert.match(html, /script-src 'self'/);
  assert.ok(!/'unsafe-inline'/.test(html), 'the CSP must not allow inline script or style');
  assert.match(html, /<script type="module" src="app\.js">/);
  // Every overlay root the renderer fills must exist in the shell.
  for (const id of ['main', 'nav', 'drawer', 'drawer-mask', 'modal', 'modal-mask', 'notif-stack', 'exec-bar']) {
    assert.match(html, new RegExp(`id="${id}"`), `index.html is missing #${id}`);
  }
});

test('style.css keeps the design system tokens and component classes', () => {
  const css = fs.readFileSync(path.join(RENDERER, 'style.css'), 'utf8');
  for (const token of ['--animal-primary', '--animal-bg-content', '--animal-radius-pill', '--tile-blue']) {
    assert.ok(css.includes(token), `missing design token ${token}`);
  }
  for (const cls of ['.btn', '.card', '.tag', '.ribbon', '.route', '.stone', '.lnode', '.drawer', '.modal', '.notif']) {
    assert.ok(new RegExp(`\\${cls}[\\s,{:]`).test(css), `missing component class ${cls}`);
  }
  // The font is bundled, not fetched: `font-src 'self'`.
  assert.match(css, /fonts\/nunito\/Nunito-Variable\.ttf/);
  assert.ok(!/fonts\.googleapis\.com/.test(css), 'the stylesheet must not reference a remote font');
});

test('the interface is in Chinese: every screen carries its title copy', () => {
  const expected = {
    'dashboard.js': '仪表盘',
    'projects.js': '我的项目',
    'project.js': '未完成',
    'task.js': '停顿面板',
    'routing.js': '路由图',
    'settings.js': '全局设置',
    'env.js': '本地环境',
    'skills.js': '技能',
  };
  for (const [file, copy] of Object.entries(expected)) {
    const source = fs.readFileSync(path.join(RENDERER, 'screens', file), 'utf8');
    assert.ok(source.includes(copy), `${file} is missing the design document's copy "${copy}"`);
  }
});
