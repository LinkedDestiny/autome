import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { expect, test } from 'vitest';
import { checkNodeVersion, MIN_NODE_VERSION } from '../server/node-version.js';

const cwd = fileURLToPath(new URL('..', import.meta.url));
function rejectVersion(version: string) {
  return spawnSync(process.execPath, [
    '--import', `./test/fixtures/fake-node-${version}.mjs`, 'server/start.js',
  ], { cwd, encoding: 'utf8', timeout: 10_000, env: { ...process.env, AUTOME_WEB_PORT: '0' } });
}

for (const [version, accepted] of [
  ['9.9.0', false], ['21.7.3', false], ['22.9.0', false], ['22.21.1', false],
  ['22.22.0', true], ['22.23.1', true], ['26.5.0', true], ['abc', false],
] as const) {
  test(`版本 ${version} → ${accepted}`, () => {
    expect(checkNodeVersion(version)).toBe(accepted);
    if (version === 'abc') {
      for (const invalid of ['', '22', '22.22', '22.22.0.1', '-22.22.0', '22.22.0-beta', ' 22.22.0', '22.22.0\n', 'Infinity.0.0', '9007199254740992.0.0']) {
        expect(checkNodeVersion(invalid), invalid).toBe(false);
      }
    }
    if (version === '22.22.0') {
      for (const boundary of ['022.022.000', '22.22.1', '23.0.0', '9007199254740991.0.0']) {
        expect(checkNodeVersion(boundary), boundary).toBe(true);
      }
      expect(checkNodeVersion('22.21.999')).toBe(false);
      expect(checkNodeVersion('0.0.0')).toBe(false);
    }
  });
}

test('拒绝消息为中文且包含当前版本与下限', () => {
  const result = rejectVersion('21.7.3');
  expect(result.error).toBeUndefined();
  expect(result.stderr.trim()).toBe('Autome Web 需要 Node.js 22.22.0 或更高版本，当前是 21.7.3。请升级 Node.js 后重试。');
});

for (const version of ['21.7.3', '22.21.1']) {
  test(`进程入口拒绝 ${version} 且不启动`, () => {
    const result = rejectVersion(version);
    expect(result.error).toBeUndefined();
    expect(result.status).toBe(1);
    expect(result.signal).toBeNull();
    expect(result.stderr).toContain(`当前是 ${version}。请升级 Node.js 后重试。`);
    expect(result.stdout).not.toContain('已启动');
  });
}

test('纯 JS 入口先检查版本，依赖结构与 engines、启动脚本一致', () => {
  const read = (path: string) => readFileSync(new URL(path, new URL('../', import.meta.url)), 'utf8');
  const entry = ts.createSourceFile('start.js', read('server/start.js'), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  const imports = entry.statements.filter(ts.isImportDeclaration);
  expect(imports.map(node => (node.moduleSpecifier as ts.StringLiteral).text)).toEqual(['./node-version.js']);
  const versionModule = ts.createSourceFile('node-version.js', read('server/node-version.js'), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  const moduleImports: ts.Node[] = [];
  function visit(node: ts.Node) {
    if (ts.isImportDeclaration(node) || (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword)) moduleImports.push(node);
    ts.forEachChild(node, visit);
  }
  visit(versionModule);
  expect(moduleImports).toEqual([]);
  const guard = entry.statements.findIndex(node => ts.isIfStatement(node) && node.expression.getText(entry).includes('checkNodeVersion(process.versions.node)') && node.getText(entry).includes('process.exit(1)'));
  const load = entry.statements.findIndex(node => node.getText(entry).includes("import('./main.ts')"));
  expect(guard).toBeGreaterThanOrEqual(0);
  expect(load).toBeGreaterThan(guard);
  expect(MIN_NODE_VERSION).toBe('22.22.0');
  const pkg = JSON.parse(read('package.json'));
  expect(pkg.engines.node).toBe(`>=${MIN_NODE_VERSION}`);
  expect(pkg.scripts.dev).toBe('node server/start.js');
  expect(pkg.scripts.start).toBe('NODE_ENV=production node server/start.js');
});
