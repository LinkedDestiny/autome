import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { expect, it } from 'vitest';
import { LiveBanner } from '../app/lib/live.tsx';
it('断线时显示顶部中文状态提示', () => {
  const html = renderToString(createElement(LiveBanner, { connection: 'reconnecting' }));
  expect(html).toContain('role="status"'); expect(html).toContain('连接中断，正在重连'); expect(html).toContain('top:0');
});
it('首次连接及在线时不显示提示', () => {
  for (const connection of ['connecting', 'open'] as const) expect(renderToString(createElement(LiveBanner, { connection }))).toBe('');
});
