import { createElement as h } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { readFileSync, readdirSync } from 'node:fs';
import { expect, test } from 'vitest';
import { ClockProvider, formatClock, formatDuration, formatRange, type Clock } from '../app/lib/clock.tsx';
import { TaskTimeline } from '../app/components/task-timeline.tsx';
import { BellView } from '../app/components/bell.tsx';
import { ProjectAlerts } from '../app/components/project-alerts.tsx';
const clock: Clock = {now:'2026-11-14T06:28:00Z',timeZone:'Asia/Shanghai'};
for (const timeZone of ['Asia/Shanghai','America/New_York']) test(`时间格式表 ${timeZone}`, () => {
  const c = {...clock,timeZone};
  const hours = timeZone === 'Asia/Shanghai' ? '14:28' : '01:28';
  for (const [iso,expected] of [['2026-11-14T06:28:00Z',hours],['2026-11-13T06:28:00Z',`11月13日 ${hours}`],['2025-11-13T06:28:00Z',`2025年11月13日 ${hours}`]]) expect(formatClock(iso,c)).toBe(expected);
  const midnight = timeZone === 'Asia/Shanghai' ? ['2026-11-13T15:59:00Z','2026-11-13T16:01:00Z'] : ['2026-11-14T04:59:00Z','2026-11-14T05:01:00Z'];
  expect(formatRange(midnight[0],midnight[1],c)).toBe('11月13日 23:59 → 00:01');
  expect(formatRange('2025-12-31T15:00:00Z','2025-12-31T15:30:00Z',c)).toMatch(/ → \d{2}:30$/);
  expect(formatClock('',c)).toBe(''); expect(formatClock('invalid',c)).toBe('invalid');
  expect(formatClock('2024-02-29T06:28:00Z',c)).toBe(`2024年2月29日 ${hours}`);
});
test('用时边界、分钟精度与异常值', () => {
  for (const [seconds,expected] of [[0,'0 秒'],[59,'59 秒'],[60,'1 分'],[530,'8 分 50 秒'],[3599,'59 分 59 秒'],[3600,'1 小时'],[3660,'1 小时 1 分'],[86399,'23 小时 59 分'],[86400,'1 天 0 小时'],[90000,'1 天 1 小时'],[-1,'0 秒'],[NaN,'0 秒'],[Infinity,'0 秒']] as const) expect(formatDuration(seconds*1000)).toBe(expected);
  expect(formatDuration(530000,'minute')).toBe('8 分'); expect(formatDuration(59000,'minute')).toBe('0 分');
});
const session = {id:'s',label:'实现',round:1,running:false,started_at:'2026-11-14T06:00:00Z',ended_at:'2026-11-14T06:08:50Z'};
const timeline = () => h(TaskTimeline,{sessions:[session],events:[],now:clock.now});
function render(child: ReturnType<typeof h>) {
  const router = createMemoryRouter([{path:'/',element:h(ClockProvider,{initial:clock},child)}]);
  try { return renderToString(h(RouterProvider,{router})); } finally {router.dispose();}
}
test('相同 loader 快照在两个进程 TZ 下 SSR 标记逐字相同', () => {
  const original = process.env.TZ;
  try {process.env.TZ='UTC'; const utc=render(timeline()); process.env.TZ='Asia/Tokyo'; expect(render(timeline())).toBe(utc); expect(utc).toContain('14:00');}
  finally { if(original===undefined) delete process.env.TZ; else process.env.TZ=original; }
});
test('全站只能在 clock 模块读取时钟或本地日期', () => {
  for (const file of readdirSync('app',{recursive:true}) as string[]) if (/\.tsx?$/.test(file) && file!=='lib/clock.tsx') expect(readFileSync('app/'+file,'utf8'),file).not.toMatch(/new Date\(|Date\.now\(|getHours\(|toLocale/);
});
test('时间线使用 provider 时区', () => {expect(render(timeline())).toContain('14:00 → 14:08');});
test('铃铛使用 provider 时区', () => {
  const html = render(h(BellView,{snapshot:{unread:1,items:[{id:'n',kind:'future',level:'info',project_id:null,task_id:null,title:'通知',body:'',created_at:clock.now,read_at:null}]},open:true,onToggle:()=>{},onRead:async()=>{},onAll:async()=>{}}));
  expect(html).toContain('>14:28</time>');
});
test('项目报警使用 provider 时区', () => {
  const html = render(h(ProjectAlerts,{alerts:[{id:'a',project_id:'p',task_id:null,kind:'setup_failed',detail:'失败',created_at:clock.now}]}));
  expect(html).toContain('>14:28</time>');
});
