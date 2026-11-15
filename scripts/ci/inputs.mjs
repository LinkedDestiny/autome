// Content-addressed gate planning. Shell remains responsible for running commands.
import { appendFileSync, copyFileSync, existsSync, readFileSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import { resolve, relative, isAbsolute } from 'node:path';
import { execFileSync } from 'node:child_process';
import { quickCandidates, display } from './quick.mjs';

const [mode, file, ...args] = process.argv.slice(2);
const git = (args, options = {}) => execFileSync('git', args, { encoding: 'utf8', ...options }).trimEnd();
const quote = value => `'${value.replaceAll("'", "'\\''")}'`;
const words = value => value.trim().split(/\s+/).filter(Boolean);
const matches = (path, prefix) => path === prefix || path.startsWith(`${prefix}/`);
const hash = value => git(['hash-object', '--stdin'], { input: value });

function snapshot() {
  const index = resolve(file, '../index');
  const actual = git(['rev-parse', '--git-path', 'index']);
  if (existsSync(actual)) copyFileSync(actual, index);
  const env = { ...process.env, GIT_INDEX_FILE: index };
  git(['add', '-A'], { env });
  const tree = git(['write-tree'], { env });
  const entries = git(['ls-tree', '-rz', '--full-tree', tree]).split('\0').filter(Boolean).map(line => {
    const tab = line.indexOf('\t');
    return { path: line.slice(tab + 1), object: line.slice(0, tab) };
  });
  return { tree, entries };
}

function toolchain() {
  const commands = [['rustc', '-vV'], ['cargo', '-V'], ['cargo', 'clippy', '-V'], ['rustfmt', '-V'],
    ['node', '-v'], ['npm', '-v'], ['git', '--version'], ['uname', '-sr']];
  const versions = commands.map(([command, ...args]) => execFileSync(command, args, { encoding: 'utf8' }));
  if (process.platform === 'darwin') versions.push(execFileSync('sw_vers', ['-productVersion'], { encoding: 'utf8' }));
  versions.push(process.env.RUSTFLAGS ?? '', process.env.NODE_OPTIONS ?? '');
  for (const path of ['apps/web/node_modules/.package-lock.json', 'apps/desktop/node_modules/.package-lock.json']) {
    versions.push(existsSync(path) ? git(['hash-object', path]) : 'absent');
  }
  return hash(JSON.stringify(versions));
}

function calculate(rows) {
  const { tree, entries } = snapshot();
  const globals = words(process.env.GLOBAL_INPUTS ?? '');
  const claimed = [...globals, ...words(process.env.NO_GATE ?? ''), ...rows.filter(r => r.kind === 'step').flatMap(r => words(r.inputs))];
  const unclaimed = entries.filter(e => !claimed.some(p => matches(e.path, p)));
  const unclaimedHash = hash(JSON.stringify(unclaimed));
  const tool = toolchain();
  return { unclaimed, keys: rows.map(row => {
    if (row.kind !== 'step') return null;
    const inputs = [...new Set([...globals, ...words(row.inputs)])].sort().map(path => {
      // ls-tree accepts literal pathspecs, including absent prefixes.
      const entry = git(['ls-tree', tree, '--', `:(literal)${path}`]);
      return [path, entry || 'absent'];
    });
    return hash(`ci-local-key 1\nstep ${row.name}\ncmd ${JSON.stringify(row.command)}\ntool ${tool}\nunclaimed ${unclaimedHash}\ninput ${JSON.stringify(inputs)}\n`);
  }) };
}

function passes() {
  const path = process.env.AUTOME_GATE_PASSES;
  if (!path) return new Map();
  try {
    const actual = realpathSync(path);
    const root = realpathSync(git(['rev-parse', '--show-toplevel']));
    const rel = relative(root, actual);
    if (!rel || (!isAbsolute(rel) && rel !== '..' && !rel.startsWith('../'))) {
      console.error('通过记录位于工作区内，不采信');
      return new Map();
    }
    if (!statSync(actual).isFile()) throw new Error('不是普通文件');
    const result = new Map();
    for (const line of readFileSync(actual, 'utf8').split('\n').filter(Boolean)) {
      const fields = line.split('\t');
      if (fields.length !== 5 || !/^[a-f0-9]{40,64}$/.test(fields[0]) || fields.some(f => /[\r\n]/.test(f))) throw new Error('格式错误');
      const [key, task, node, head, time] = fields;
      if (!result.has(key)) result.set(key, `${task}/${node}@${head} ${time}`);
    }
    return result;
  } catch (error) {
    console.error(`无法读取通过记录，不采信：${error.message}`);
    return new Map();
  }
}

try {
  if (mode === 'declare') {
    const [kind, name, inputs, ...command] = args;
    if (/[\t\r\n]/.test(name)) throw new Error('步骤名不能含 Tab 或换行');
    appendFileSync(file, `${JSON.stringify({ kind, name, inputs, command })}\n`);
  } else {
    const rows = readFileSync(file, 'utf8').trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
    const { keys, unclaimed } = calculate(rows);
    if (mode === 'key') {
      console.log(keys[Number(args[0])]);
    } else if (mode === 'claims') {
      for (const { path } of unclaimed) console.log(path);
      process.exitCode = unclaimed.length ? 1 : 0;
    } else {
      const records = passes();
      const reused = keys.map(key => process.env.CI_LOCAL_ALL !== '1' && records.has(key));
      if (mode === 'quick-plan' || mode === 'quick-prepare') {
        const candidates = quickCandidates();
        const lines = [];
        const planning = mode === 'quick-plan';
        const emit = command => lines.push(command.map(quote).join(' '));
        const prepare = name => {
          const row = rows.find(r => r.kind === 'prep' && r.name === name);
          if (!row) throw new Error(`缺少准备步骤 ${name}`);
          if (planning) console.log(`prep\t${name}\t${display(row.command)}`);
          else emit(['execute_prep', row.name, ...row.command]);
        };
        const full = name => {
          const i = rows.findIndex(r => r.kind === 'step' && r.name === name);
          if (i < 0) throw new Error(`缺少完整步骤 ${name}`);
          const row = rows[i];
          if (planning) console.log(`plan\t${reused[i] ? '沿用' : '执行'}\t${keys[i]}\t${name}`);
          else emit(['execute_step', String(i), keys[i], reused[i] ? records.get(keys[i]) : '', name, ...row.command]);
        };
        full('Format'); full('Clippy');
        if (rows.some((r, i) => ['Web lint', 'Web typecheck'].includes(r.name) && !reused[i])) prepare('Web install');
        full('Web lint'); full('Web typecheck');
        emit(['ci_quick_start']);
        for (const candidate of candidates) {
          if (candidate.omitted) {
            if (planning) console.log(`未跑\t${candidate.omitted}`);
            else emit(['ci_not_run', candidate.omitted]);
            continue;
          }
          const name = candidate.full ?? display(candidate.command);
          if (!planning) lines.push(`if ci_quick_admit ${quote(name)}; then`);
          const fullIndex = rows.findIndex(r => r.kind === 'step' && r.name === candidate.full);
          if (!candidate.full || !reused[fullIndex]) {
            for (const prep of candidate.prep ?? []) prepare(prep);
          }
          if (candidate.full) full(candidate.full);
          else if (planning) console.log(`candidate\t${JSON.stringify(candidate.command)}`);
          else if (candidate.web) emit(['ci_web_related', ...candidate.web]);
          else emit(['ci_partial', name, '--cases', candidate.parser, ...candidate.command]);
          if (!planning) lines.push('fi');
        }
        if (!planning) writeFileSync(args[0], `${lines.join('\n')}\n`);
      } else if (mode === 'plan') {
        rows.forEach((row, i) => {
          if (row.kind === 'step') console.log(`plan\t${reused[i] ? '沿用' : '执行'}\t${keys[i]}\t${row.name}`);
        });
      } else if (mode === 'prepare') {
        const lines = [];
        rows.forEach((row, i) => {
          if (row.kind === 'prep') {
            if (rows.some((later, j) => j > i && later.kind === 'step' && !reused[j])) {
              lines.push(['execute_prep', row.name, ...row.command].map(quote).join(' '));
            }
          } else {
            lines.push(['execute_step', String(i), keys[i], reused[i] ? records.get(keys[i]) : '', row.name, ...row.command].map(quote).join(' '));
          }
        });
        writeFileSync(args[0], `${lines.join('\n')}\n`);
      } else throw new Error(`未知模式 ${mode}`);
    }
  }
} catch (error) {
  console.error(`ci inputs: ${error.message}`);
  process.exitCode = 2;
}
