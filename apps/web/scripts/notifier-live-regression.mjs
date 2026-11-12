// Run with Node >=22.22; no npm install, Rust build, network or wall-clock sleeps.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { promisify } from 'node:util';
import vm from 'node:vm';

const source = readFileSync(new URL('../test/notifier-live.test.ts', import.meta.url), 'utf8');
function load(execFile) {
  const cases = [];
  const register = (name, action, timeout) => cases.push({ name, action, timeout });
  const context = vm.createContext({
    test: register, beforeAll() {}, afterAll() {},
    mkdtempSync: () => '/virtual/notifier', tmpdir: () => '/virtual',
    join: (...parts) => parts.join('/'), resolve: (...parts) => parts.join('/'),
    process: { env: {} }, promisify, execFile,
  });
  // Evaluate the actual helper and registrations, not copied deadline constants.
  // Imports and import.meta alone are replaced; hooks/test bodies are not run.
  const code = stripTypeScriptTypes(source.replace(/^import .*;\r?\n/gm, '')
    .replaceAll('import.meta.dirname', JSON.stringify('/virtual/apps/web/test')));
  vm.runInContext(code + '\n globalThis.invoke = command;', context);
  return { command: context.invoke, cases };
}

test('notifier command accepts a successful reply after 20 virtual seconds', async t => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const { command } = load((file, args, options, callback) => {
    // Model execFile: successful child completion wins only if its deadline
    // has not killed it. Schedule both events on Node's virtual timer queue.
    let settled = false;
    setTimeout(() => {
      if (settled) return;
      settled = true;
      callback(null, 'reply arrived', '');
    }, 20_000);
    if (options.timeout > 0) setTimeout(() => {
      if (settled) return;
      settled = true;
      callback(Object.assign(new Error('Command failed: slow-kernel'), {
        killed: true, signal: 'SIGTERM', code: null,
      }), '', '');
    }, options.timeout);
    return {};
  });
  const outcome = command('slow-kernel', []).then(
    reply => ({ reply }), error => ({ error }),
  );
  t.mock.timers.tick(20_000);
  const result = await outcome;
  assert.equal(result.error, undefined,
    'a healthy 20s command was killed by the notifier-live execFile deadline');
  assert.equal(result.reply, 'reply arrived');
});

test('offline restart case budgets two successful 20s commands plus replay', () => {
  const { cases } = load(() => { throw new Error('unexpected command'); });
  const restart = cases.find(c => c.name === '停服期间新增失败，重启只补一条且保留旧已读');
  assert.ok(restart, 'the offline restart behavioral case must remain registered');
  // task new and run execute sequentially; reserve 5s for close/start/replay.
  // An omitted override uses the checked-in Vitest config, not an assumed value.
  const config = readFileSync(new URL('../vitest.config.ts', import.meta.url), 'utf8');
  const defaultTimeout = Number(config.match(/testTimeout:\s*([\d_]+)/)?.[1].replaceAll('_', ''));
  assert.ok((restart.timeout ?? defaultTimeout) >= 45_000,
    `offline restart budget ${restart.timeout ?? defaultTimeout}ms cannot contain the 45s scenario`);
});
