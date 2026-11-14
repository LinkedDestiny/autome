import { readFileSync } from 'node:fs';

// Work in Unicode code points and UTF-8 bytes, including the ellipsis itself.
function clip(value) {
  const chars = Array.from(value);
  if (chars.length <= 160 && Buffer.byteLength(value) <= 200) return value;
  let result = '';
  for (const char of chars.slice(0, 159)) {
    if (Buffer.byteLength(result + char + '…') > 200) break;
    result += char;
  }
  return result + '…';
}

function extract(parser, output) {
  const cases = [];
  let target = 'unknown';
  let failures = false;
  for (const line of output.replace(/\x1b\[[0-9;]*m/g, '').split(/\r?\n/)) {
    if (parser === 'cargo') {
      const running = line.match(/^\s*Running (.+?) \((.+)\)$/);
      if (running) {
        const binary = running[2].split('/').at(-1).replace(/-[a-f0-9]+$/, '');
        target = running[1].startsWith('unittests src/lib.rs') ? `${binary} (lib)` : binary;
        failures = false;
      } else if (line === 'failures:') {
        failures = true;
      } else if (failures && /^ {4}\S/.test(line)) {
        cases.push(`${target}: ${line.trim()}`);
      } else if (failures && line !== '') {
        failures = false;
      }
    } else if (parser === 'vitest') {
      const failure = line.match(/^\s*FAIL\s+(.+)$/);
      if (failure) cases.push(failure[1]);
    }
  }
  return [...new Set(cases)];
}

const [mode, ...args] = process.argv.slice(2);
if (mode === 'extract') {
  const [parser, step, file] = args;
  const cases = extract(parser, readFileSync(file, 'utf8'));
  if (cases.length) console.log(JSON.stringify({ step, cases }));
} else if (mode === 'render') {
  const groups = readFileSync(args[0], 'utf8').split('\n').filter(Boolean).map(line => JSON.parse(line));
  const total = groups.reduce((sum, group) => sum + group.cases.length, 0);
  let block = '';
  let listed = 0;
  // Titles and case lines share one global budget, even with many failed steps.
  outer: for (const { step, cases } of groups) {
    const title = clip(`失败用例 · ${step}（${cases.length}）：`) + '\n';
    for (const [index, name] of cases.entries()) {
      const addition = (index === 0 ? title : '') + clip(`  - ${name}`) + '\n';
      if (listed === 20 || Buffer.byteLength(block + addition) > 2400) break outer;
      block += addition;
      listed++;
    }
  }
  if (listed < total) block += `  …另有 ${total - listed} 个，见上方日志\n`;
  process.stdout.write(block);
}
