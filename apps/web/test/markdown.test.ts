import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { parseMarkdown, safeHref } from '../app/lib/markdown.ts';
import { MarkdownView } from '../app/components/markdown-view.tsx';
const render = (text: string) => renderToStaticMarkup(createElement(MarkdownView, { nodes: parseMarkdown(text) }));
it('块级子集包括一层嵌套列表', () => {
  const html = render('# 标题\n\n段落\n\n- 一\n  - 子\n- 二\n\n1. 有序\n\n> 引用\n\n---\n\n```ts\nconst a = 1\n```');
  for (const tag of ['h1', 'p', 'ul', 'ol', 'blockquote', 'hr', 'pre']) expect(html).toContain(`<${tag}`);
  expect(html).toContain('<li>一<ul><li>子</li></ul></li>'); expect(html).toContain('const a = 1');
});
it('GFM 表格和空单元格', () => {
  expect(render('| ID | 状态 |\n| :--- | ---: |\n| M-01 | |')).toContain('<td>M-01</td><td></td>');
});
it('HTML 与 script 作为字面文本', () => {
  const html = render('<script>alert(1)</script>\n<div onclick="x">a</div>');
  expect(html).toContain('&lt;script&gt;'); expect(html).toContain('&lt;div'); expect(html).not.toContain('<script');
});
it('链接协议白名单、控制字符、大小写与相对路径', () => {
  for (const target of ['javascript:alert', 'JaVaScRiPt:alert', 'java script:alert', 'java\tscript:alert', 'java\u0000script:alert', 'data:text/html,x', 'vbscript:x', '../file.md', '#anchor', '']) {
    expect(safeHref(target)).toBeNull(); expect(render(`[text](${target})`)).not.toContain('<a ');
  }
  for (const target of ['https://example.com', 'HTTP://example.com', 'mailto:a@example.com']) {
    expect(render(`[text](${target})`)).toContain('rel="noreferrer" target="_blank"');
  }
  expect(render('[relative](./file.md)')).toContain('<code>./file.md</code>');
});
it('图片不加载', () => { expect(render('![花](https://example.com/a.png)')).toBe('<p>[图片：花]</p>'); });
it('输出与源码无 HTML 注入入口', () => {
  const html = render('<script>x</script>\n\n[x](javascript:x) ![x](data:x)');
  for (const value of ['<script', 'href="javascript', '<img']) expect(html).not.toContain(value);
  for (const file of ['app/components/markdown-view.tsx', 'app/lib/markdown.ts']) expect(readFileSync(file, 'utf8')).not.toContain('dangerouslySetInnerHTML');
});
it('行内代码、粗体、斜体与未知语法原文', () => {
  expect(render('`code` **bold** *em* __bold2__ _em2_ ~~raw~~')).toBe('<p><code>code</code> <strong>bold</strong> <em>em</em> <strong>bold2</strong> <em>em2</em> ~~raw~~</p>');
});
it('空输入、单行、截断未闭合围栏', () => {
  expect(parseMarkdown('')).toEqual([]); expect(render('x')).toBe('<p>x</p>');
  expect(render('```\n<script>\n')).toBe('<pre><code>&lt;script&gt;\n</code></pre>');
});
