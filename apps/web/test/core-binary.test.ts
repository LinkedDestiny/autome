import { test, expect } from 'vitest';
import { resolveAutomedBinary } from '../server/core/binary.ts';
const options = { env: {}, home: '/home/test', webRoot: '/repo/apps/web', exists: () => false };
test('环境覆盖优先且不检查存在', () => {
  expect(resolveAutomedBinary({ ...options, env: { AUTOMED_BIN: '/missing' }, exists: () => { throw Error('不应查盘'); } }))
    .toEqual({ path: '/missing', args: [], source: 'env' });
});
test('发布包先于本地及 cargo', () => {
  expect(resolveAutomedBinary({ ...options, exists: () => true })).toEqual({ path: '/repo/apps/automed', args: [], source: 'bundle' });
});
test('本地安装先于 cargo', () => {
  expect(resolveAutomedBinary({ ...options, exists: p => p === '/home/test/.local/bin/autome' }))
    .toEqual({ path: '/home/test/.local/bin/autome', args: ['core'], source: 'cli' });
});
test('cargo 兜底即使不存在，支持 profile', () => {
  expect(resolveAutomedBinary(options)).toEqual({ path: '/repo/target/debug/automed', args: [], source: 'cargo' });
  expect(resolveAutomedBinary({ ...options, env: { AUTOMED_CARGO_PROFILE: 'release' } }))
    .toEqual({ path: '/repo/target/release/automed', args: [], source: 'cargo' });
});
test('空覆盖变量视同未设', () => {
  expect(resolveAutomedBinary({ ...options, env: { AUTOMED_BIN: '' } })).toEqual(resolveAutomedBinary(options));
});
test('发布包 automed 与 web 目录同级，source 可追溯', () => {
  expect(resolveAutomedBinary({ ...options, webRoot: '/package/web', exists: p => p === '/package/automed' }))
    .toEqual({ path: '/package/automed', args: [], source: 'bundle' });
});

test('环境覆盖 autome 带 core', () => {
 expect(resolveAutomedBinary({ ...options, env: { AUTOMED_BIN: '/custom/autome' } })).toEqual({path:'/custom/autome', args:['core'], source:'env'});
});
test('PATH 第一个 autome 优先于缺省安装', () => {
 expect(resolveAutomedBinary({...options, env:{PATH:'/first:/second'}, exists:p=>p.endsWith('/autome')})).toEqual({path:'/first/autome',args:['core'],source:'cli'});
});
test('不再查旧 local-bin automed', () => {
 const checked:string[]=[];
 expect(resolveAutomedBinary({...options, exists:p=>{checked.push(p);return p.endsWith('.local/bin/automed');}}).source).toBe('cargo');
 expect(checked).not.toContain('/home/test/.local/bin/automed');
});
