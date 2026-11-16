// Repository-specific changed-test selection; execution stays in lib.sh.
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const git = args => execFileSync('git', args, { encoding: 'utf8' });
export const quote = value => `'${value.replaceAll("'", "'\\''")}'`;
export const display = command => command.map(quote).join(' ');

// Only accept rustfmt's top-level #[test] functions. Ambiguous edits fall back
// to an explicit omission, never to the expensive whole end_to_end target.
function testBodies(source) {
  // Do not recognize function-looking text embedded in Rust strings/comments.
  // Preserve line numbers while hiding tokens irrelevant to body boundaries.
  source = maskLiterals(source);
  const lines = source.split('\n');
  const bodies = [];
  for (let i = 0; i < lines.length; i++) {
    if (lines[i] !== '#[test]') continue;
    const match = /^fn (\w+)\(\) \{$/.exec(lines[i + 1] ?? '');
    if (!match) continue;
    let depth = 1;
    let end = -1;
    for (let j = i + 2; j < lines.length; j++) {
      for (const char of lines[j]) {
        if (char === '{') depth++;
        if (char === '}') depth--;
      }
      if (depth === 0) { end = j; break; }
    }
    if (end > i + 1) bodies.push({ name: match[1], start: i + 3, end });
  }
  return bodies;
}

function maskLiterals(source) {
  const hidden = value => value.replace(/[^\n]/g, ' ');
  let result = '';
  for (let i = 0; i < source.length;) {
    const rest = source.slice(i);
    let length = 0;
    const raw = /^r(#{0,255})"/.exec(rest);
    if (raw) {
      const close = source.indexOf(`"${raw[1]}`, i + raw[0].length);
      if (close < 0) throw new Error('未闭合的 Rust raw string');
      length = close + 1 + raw[1].length - i;
    } else if (rest.startsWith('//')) {
      const end = source.indexOf('\n', i);
      length = (end < 0 ? source.length : end) - i;
    } else if (rest.startsWith('/*')) {
      let depth = 1; let end = i + 2;
      while (end < source.length && depth) {
        if (source.startsWith('/*', end)) { depth++; end += 2; }
        else if (source.startsWith('*/', end)) { depth--; end += 2; }
        else end++;
      }
      if (depth) throw new Error('未闭合的 Rust comment');
      length = end - i;
    } else if (rest[0] === '"') {
      let end = i + 1;
      while (end < source.length && source[end] !== '"') end += source[end] === '\\' ? 2 : 1;
      if (end >= source.length) throw new Error('未闭合的 Rust string');
      length = end + 1 - i;
    } else {
      // A lifetime is not a char literal.
      length = /^'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'/u.exec(rest)?.[0].length ?? 0;
    }
    if (length) { result += hidden(source.slice(i, i + length)); i += length; }
    else { result += source[i]; i++; }
  }
  return result;
}
function changedTests(since, path) {
  const before = testBodies(git(['show', `${since}:${path}`]));
  const after = testBodies(readFileSync(path, 'utf8'));
  const diff = git(['diff', '--no-ext-diff', '--no-textconv', '--unified=0', since, '--', path]);
  const names = new Set();
  for (const match of diff.matchAll(/^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/gm)) {
    const [, oldStart, oldCount = '1', newStart, newCount = '1'] = match;
    const containing = (bodies, start, count) => bodies.find(b =>
      // A zero-length side is anchored on the preceding line, including the
      // opening brace when inserting/deleting the first statement of a test.
      Number(start) >= b.start - (Number(count) === 0 ? 1 : 0)
      && Number(start) + Math.max(Number(count), 1) - 1 <= b.end);
    const old = containing(before, oldStart, oldCount);
    const next = containing(after, newStart, newCount);
    if (!old || !next || old.name !== next.name) return [];
    names.add(next.name);
  }
  return [...names];
}

export function changedPaths() {
  const since = process.env.AUTOME_ROUND_START || process.env.AUTOME_BASE || process.env.CI_QUICK_SINCE;
  if (!since) return null;
  // Resolve explicitly: invalid revisions must not look like an empty change set.
  const revision = git(['rev-parse', '--verify', `${since}^{commit}`]).trim();
  const paths = [...new Set([
    ...git(['diff', '--no-ext-diff', '--no-renames', '--name-only', '-z', revision, '--']).split('\0'),
    ...git(['ls-files', '--others', '--exclude-standard', '-z']).split('\0'),
  ].filter(Boolean))].sort();
  if (paths.some(path => /[\t\r\n]/.test(path))) throw new Error('改动路径不能含 Tab 或换行');
  return paths;
}

export function quickCandidates() {
  const paths = changedPaths();
  if (!paths) return [{ omitted: '未提供本轮起点，未跑改动相关的测试' }];
  const revision = process.env.AUTOME_ROUND_START || process.env.AUTOME_BASE || process.env.CI_QUICK_SINCE;
  const candidates = [];
  const add = (priority, command, parser = 'cargo', prep = []) => {
    // Integration targets can launch sibling binaries that cargo test -p does not rebuild.
    if (parser === 'cargo' && (command.includes('--test') || command.includes('--tests'))) prep = ['Build core', ...prep];
    candidates.push({ priority, command, parser, prep });
  };
  let metadata;
  const unit = (crate, priority) => {
    metadata ??= JSON.parse(execFileSync('cargo', ['metadata', '--no-deps', '--format-version', '1'], { encoding: 'utf8' }));
    const pkg = metadata.packages.find(p => p.name === crate);
    if (!pkg) throw new Error(`无法识别 crate ${crate}`);
    const kind = pkg.targets.some(t => t.kind.some(k => ['lib', 'rlib', 'proc-macro'].includes(k))) ? '--lib' : '--bins';
    add(priority, ['cargo', 'test', '-p', crate, kind, '--no-fail-fast']);
  };
  for (const path of paths) {
    if (path === 'crates/automed/tests/end_to_end.rs') {
      let names = [];
      try { names = changedTests(revision, path); } catch { /* deleted/new/ambiguous file */ }
      if (names.length) add(1, ['cargo', 'test', '-p', 'automed', '--test', 'end_to_end', '--no-fail-fast', '--', ...names]);
      else candidates.push({ priority: 1, omitted: 'cargo test -p automed --test end_to_end（改动不全在既有测试函数体内，留给合并前门禁）' });
    } else {
      const integration = /^crates\/([^/]+)\/tests\/([^/]+)\.rs$/.exec(path);
      if (integration) add(1, ['cargo', 'test', '-p', integration[1], '--test', integration[2], '--no-fail-fast']);
      else if (/^crates\/[^/]+\/tests\//.test(path)) add(2, ['cargo', 'test', '-p', path.split('/')[1], '--tests', '--no-fail-fast']);
    }
    const source = /^crates\/([^/]+)\/(?:src\/|build[^/]*\.rs$|Cargo\.toml$)/.exec(path);
    if (source) unit(source[1], 2);
    if (path.startsWith('crates/autome-domain/src/')) {
      unit('automed', 3); unit('autome-cli', 3);
    }
  }
  if (paths.some(p => p.startsWith('protocol/'))) {
    add(2, ['cargo', 'test', '-p', 'automed', '--lib', '--no-fail-fast', 'protocol']);
    add(2, ['cargo', 'test', '-p', 'automed', '--test', 'prompt_rendering', '--no-fail-fast']);
  }
  const web = paths.filter(p => p.startsWith('apps/web/')).map(p => p.slice('apps/web/'.length));
  // Vitest 5 treats tokens after -- as runner arguments, not related paths.
  if (web.length) {
    add(1, ['sh', '-c', `cd apps/web && npx vitest related --run ${web.map(path => quote(path.startsWith('-') ? `./${path}` : path)).join(' ')}`], 'vitest', ['Web install', 'Build core']);
    candidates.at(-1).web = web;
  }
  if (paths.some(p => p.startsWith('apps/desktop/'))) candidates.push({ priority: 1, full: 'Desktop', prep: ['Desktop install', 'Build core'] });
  const seen = new Set();
  return candidates.sort((a, b) => a.priority - b.priority).filter(candidate => {
    const key = JSON.stringify(candidate.command ?? candidate.full ?? candidate.omitted);
    if (seen.has(key)) return false;
    seen.add(key); return true;
  });
}
