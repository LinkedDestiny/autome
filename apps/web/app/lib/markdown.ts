export type Inline = { type: 'text' | 'code'; text: string } | { type: 'strong' | 'em'; children: Inline[] } | { type: 'link'; text: string; href: string; relative: boolean };
export type MdNode =
  | { type: 'heading'; level: number; text: string }
  | { type: 'paragraph' | 'quote'; text: string }
  | { type: 'code'; text: string }
  | { type: 'rule' }
  | { type: 'list'; ordered: boolean; items: { text: string; children: MdNode[] }[] }
  | { type: 'table'; header: string[]; rows: string[][]; milestone: boolean };

export function safeHref(target: string): string | null {
  const normalized = Array.from(target).filter(char => char.charCodeAt(0) > 32 && char.charCodeAt(0) !== 127 && !/\s/.test(char)).join('');
  return /^(https?:|mailto:)/i.test(normalized) ? normalized : null;
}
export function parseInline(text: string): Inline[] {
  const nodes: Inline[] = [];
  const pattern = /!\[([^\]]*)\]\(([^)]*)\)|\[([^\]]*)\]\(([^)]*)\)|`([^`]+)`|\*\*([^*]+)\*\*|__([^_]+)__|\*([^*]+)\*|_([^_]+)_/g;
  let offset = 0;
  for (const match of text.matchAll(pattern)) {
    if (match.index > offset) nodes.push({ type: 'text', text: text.slice(offset, match.index) });
    if (match[1] !== undefined) nodes.push({ type: 'text', text: `[图片：${match[1]}]` });
    else if (match[3] !== undefined) nodes.push({ type: 'link', text: match[3], href: match[4], relative: !/^[\w\s]+:/.test(match[4]) });
    else if (match[5]) nodes.push({ type: 'code', text: match[5] });
    else nodes.push({ type: match[6] || match[7] ? 'strong' : 'em', children: [{ type: 'text', text: match[6] ?? match[7] ?? match[8] ?? match[9] }] });
    offset = match.index + match[0].length;
  }
  if (offset < text.length) nodes.push({ type: 'text', text: text.slice(offset) });
  return nodes;
}
const cells = (line: string) => line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map(cell => cell.trim());
const listItem = /^( *)([-+*]|\d+\.)\s+(.*)$/;
const divider = (line: string) => cells(line).every(cell => /^:?-{3,}:?$/.test(cell));
export function parseMarkdown(text: string): MdNode[] {
  const lines = text.replace(/\r\n?/g, '\n').split('\n');
  const nodes: MdNode[] = [];
  let milestone = false;
  for (let i = 0; i < lines.length;) {
    const line = lines[i];
    if (!line.trim()) { i++; continue; }
    const heading = /^(#{1,6})\s+(.+?)\s*#*\s*$/.exec(line);
    const fence = /^\s*(`{3,}|~{3,})/.exec(line);
    if (heading) {
      if (heading[1].length <= 2) milestone = heading[1].length === 2 && heading[2] === '里程碑';
      nodes.push({ type: 'heading', level: heading[1].length, text: heading[2] }); i++;
    } else if (fence) {
      const body: string[] = []; i++;
      const close = new RegExp(`^\\s*${fence[1][0]}{${fence[1].length},}\\s*$`);
      while (i < lines.length && !close.test(lines[i])) body.push(lines[i++]);
      if (i < lines.length) i++;
      nodes.push({ type: 'code', text: body.join('\n') });
    } else if (/^\s*(?:-{3,}|\*{3,}|_{3,})\s*$/.test(line)) { nodes.push({ type: 'rule' }); i++;
    } else if (line.includes('|') && i + 1 < lines.length && divider(lines[i + 1])) {
      const header = cells(line), rows: string[][] = []; i += 2;
      while (i < lines.length && lines[i].includes('|') && lines[i].trim()) rows.push(cells(lines[i++]));
      nodes.push({ type: 'table', header, rows, milestone });
    } else if (listItem.test(line)) {
      const first = listItem.exec(line)!;
      const items: { text: string; children: MdNode[] }[] = [];
      const ordered = /\d/.test(first[2]);
      while (i < lines.length) {
        const item = listItem.exec(lines[i]);
        if (!item || item[1].length !== first[1].length || /\d/.test(item[2]) !== ordered) break;
        i++;
        const nested: string[] = [];
        while (i < lines.length && /^\s+\S/.test(lines[i]) && (listItem.exec(lines[i])?.[1].length ?? 0) > first[1].length) nested.push(lines[i++].trimStart());
        items.push({ text: item[3], children: nested.length ? parseMarkdown(nested.join('\n')) : [] });
      }
      nodes.push({ type: 'list', ordered, items });
    } else if (/^>\s?/.test(line)) {
      const body: string[] = [];
      while (i < lines.length && /^>\s?/.test(lines[i])) body.push(lines[i++].replace(/^>\s?/, ''));
      nodes.push({ type: 'quote', text: body.join('\n') });
    } else {
      const body = [line]; i++;
      while (i < lines.length && lines[i].trim() && !/^(?:#{1,6}\s|\s*```|\s*~~~|>|\s*[-+*]\s|\s*\d+\.\s|---+$|___+$|\*\*\*+$)/.test(lines[i]) && !(lines[i].includes('|') && i + 1 < lines.length && divider(lines[i + 1]))) body.push(lines[i++]);
      nodes.push({ type: 'paragraph', text: body.join('\n') });
    }
  }
  return nodes;
}
