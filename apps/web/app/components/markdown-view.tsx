import { createElement, Fragment } from 'react';
import { parseInline, safeHref, type Inline, type MdNode } from '../lib/markdown.ts';
function InlineView({ nodes }: { nodes: Inline[] }) {
  return nodes.map((node, i) => {
    if (node.type === 'text') return <Fragment key={i}>{node.text}</Fragment>;
    if (node.type === 'code') return <code key={i}>{node.text}</code>;
    if (node.type === 'strong' || node.type === 'em') return createElement(node.type, { key: i }, <InlineView nodes={node.children} />);
    if (node.type === 'link') {
      const href = safeHref(node.href);
      return href ? <a className="link link-ext" key={i} href={href} rel="noreferrer" target="_blank">{node.text}</a>
        : <Fragment key={i}>{node.text}{node.relative && <> <code>{node.href}</code></>}</Fragment>;
    }
  });
}
const inline = (text: string) => <InlineView nodes={parseInline(text)} />;
export function MarkdownView({ nodes, currentMilestone }: { nodes: MdNode[]; currentMilestone?: string | null }) {
  return nodes.map((node, i) => {
    switch (node.type) {
      case 'heading': return createElement(`h${node.level}`, { key: i }, inline(node.text));
      case 'paragraph': return <p key={i}>{inline(node.text)}</p>;
      case 'quote': return <blockquote key={i}>{inline(node.text)}</blockquote>;
      case 'code': return <pre key={i}><code>{node.text}</code></pre>;
      case 'rule': return <hr key={i} />;
      case 'list': return createElement(node.ordered ? 'ol' : 'ul', { key: i }, node.items.map((item, j) => <li key={j}>{inline(item.text)}<MarkdownView nodes={item.children} /></li>));
      case 'table': return <table key={i}><thead><tr>{node.header.map((cell, j) => <th key={j}>{inline(cell)}</th>)}</tr></thead><tbody>{node.rows.map((row, j) => {
        const current = node.milestone && !!currentMilestone && row[0] === currentMilestone;
        return <tr key={j} className={current ? 'is-current' : undefined} aria-current={current ? 'true' : undefined}>{row.map((cell, k) => <td key={k}>{inline(cell)}</td>)}</tr>;
      })}</tbody></table>;
    }
  });
}
