import { test, expect } from 'vitest';
import { resolveAutomedBinary } from '../server/core/binary.ts';
const options = { env: {}, home: '/home/test', webRoot: '/repo/apps/web', exists: () => false };
test('环境覆盖优先且不检查存在', () => {
  expect(resolveAutomedBinary({ ...options, env: { AUTOMED_BIN: '/missing' }, exists: () => { throw Error('不应查盘'); } }))
    .toEqual({ path: '/missing', source: 'env' });
});
test('发布包先于本地及 cargo', () => {
  expect(resolveAutomedBinary({ ...options, exists: () => true })).toEqual({ path: '/repo/apps/automed', source: 'bundle' });
});
test('本地安装先于 cargo', () => {
  expect(resolveAutomedBinary({ ...options, exists: p => p === '/home/test/.local/bin/automed' }))
    .toEqual({ path: '/home/test/.local/bin/automed', source: 'local-bin' });
});
test('cargo 兜底即使不存在，支持 profile', () => {
  expect(resolveAutomedBinary(options)).toEqual({ path: '/repo/target/debug/automed', source: 'cargo' });
  expect(resolveAutomedBinary({ ...options, env: { AUTOMED_CARGO_PROFILE: 'release' } }))
    .toEqual({ path: '/repo/target/release/automed', source: 'cargo' });
});
test('空覆盖变量视同未设', () => {
  expect(resolveAutomedBinary({ ...options, env: { AUTOMED_BIN: '' } })).toEqual(resolveAutomedBinary(options));
});
test('发布包 automed 与 web 目录同级，source 可追溯', () => {
  expect(resolveAutomedBinary({ ...options, webRoot: '/package/web', exists: p => p === '/package/automed' }))
    .toEqual({ path: '/package/automed', source: 'bundle' });
});
