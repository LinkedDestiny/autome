import { timedExpect } from './harness/budget.ts';
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { test, expect, visit } from './harness/browser.ts';
import { checkout, cli, flowSteps, liveState, newTask, state } from './harness/fixture.ts';

test('B-1 网页留备注后实现轮简报含它且页面标已送达', async ({ page }) => {
  const f=state(), slug='browser-notes';flowSteps(f.root,slug);const id=newTask(f.root,slug);
  await visit(page,`/tasks/${id}`);
  await expect(page.getByRole('button',{name:'批准',exact:true})).toBeVisible({timeout:60_000});
  await page.getByRole('link',{name:'备注与依赖',exact:true}).click();
  await page.getByLabel('备注正文').fill('实现时保留这条网页备注');
  await page.getByRole('button',{name:'提交备注',exact:true}).click();
  await expect(page.getByRole('region',{name:'备注',exact:true})).toContainText('等待下一轮');
  await page.getByRole('button',{name:'批准',exact:true}).click();
  await expect(page.getByRole('region',{name:'备注',exact:true})).toContainText('已送达 · 实现 第 1 轮',{timeout:60_000});
  expect(readFileSync(join(f.repo,'.worktree',slug,'docs',slug,'brief/impl-1.md'),'utf8')).toContain('实现时保留这条网页备注');
  await expect(page.getByLabel('同时送回实现轮处理（不合并）')).toBeVisible({timeout:60_000});
});

test('B-4 命令行备注五秒内出现在详情页', async ({ page }) => {
  const f=liveState();expect(f.tickIntervalMs).toBe(3000);const id=newTask(f.root,'cli-notes',['--draft']);
  let loads=0;page.on('load',()=>loads++);
  await page.goto(`${f.url}/tasks/${id}?tab=notes`);
  await expect(page.getByRole('button',{name:'提交备注',exact:true})).toBeEnabled();
  cli(f.root,['note',id,'命令行备注实时显示']);const start=performance.now();
  await expect(page.getByRole('region',{name:'备注',exact:true})).toContainText('命令行备注实时显示',{timeout:5000});
  expect(performance.now()-start).toBeLessThanOrEqual(5000);expect(loads).toBe(1);
});

function taskState(root: string, id: string) {
  return JSON.parse(cli(root, ['rpc', 'task.get', JSON.stringify({ task_id: id })])).task.state as { state: string; node?: string };
}
function heldProject(root: string, name: string) {
  const repo = join(root, name);
  execFileSync('git', ['clone', '--quiet', join(root, 'repo'), repo]);
  for (const [key, value] of [['user.name', 'test'], ['user.email', 'test@example.test']]) execFileSync('git', ['-C', repo, 'config', key, value]);
  const config = join(repo, '.autome/config.toml');
  writeFileSync(config, readFileSync(config, 'utf8').replace('parallel = 4', 'parallel = 1'));
  execFileSync('git', ['-C', repo, 'add', '.autome/config.toml']);
  execFileSync('git', ['-C', repo, 'commit', '--quiet', '-m', 'single slot']);
  cli(root, ['project', 'add', repo, '--skip-onboarding']);
  const projects = JSON.parse(cli(root, ['rpc', 'project.list', '{}'])).projects as { project: { id: string; path: string } }[];
  const projectId = projects.find(p => p.project.path === repo || p.project.path === '/private' + repo)?.project.id;
  if (!projectId) throw Error('missing fixture project');
  function create(slug: string) {
    const dir = join(repo, '.autome/fake', slug); mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, 'next'), '1');
    writeFileSync(join(dir, '1.sh'), `while [ ! -f '${root}/release-${name}-${slug}' ]; do '${root}/bin/w7-sleep' 0.1; done\n`);
    const id = JSON.parse(cli(root, ['task', 'new', slug, '-p', projectId!, '--brief', '-', '--json'], '# ' + slug)).task.id as string;
    appendFileSync(join(root, 'task-ids'), id + '\n'); return id;
  }
  return { projectId, create };
}

test('B-2 调序后内核按新顺序启动', async ({ page }) => {
  const f = state(), p = heldProject(f.root, 'queue-order');
  const held = p.create('slot-holder');
  await expect.poll(() => taskState(f.root, held).state).toBe('active');
  const q1 = p.create('queue-first'), q2 = p.create('queue-second');
  expect(taskState(f.root, q1).state).toBe('queued'); expect(taskState(f.root, q2).state).toBe('queued');
  await visit(page, `/projects/${p.projectId}`);
  const card = page.locator(`[data-task-id="${q2}"]`);
  await card.getByText('调整顺序', { exact: true }).click();
  await card.getByRole('button', { name: '移到最前', exact: true }).click();
  await expect(card).toContainText('排队第 1 位');
  cli(f.root, ['cancel', held]);
  await expect.poll(() => taskState(f.root, q2).state).toBe('active');
  expect(taskState(f.root, q1).state).toBe('queued');
  await expect(page.locator(`[data-task-id="${q1}"]`)).toContainText('排队第 1 位');
  await page.screenshot({ path: join(checkout, '.autome/output/m04-board.png') });
  cli(f.root, ['cancel', q2]); cli(f.root, ['cancel', q1]);
});

test('B-3 依赖任务在前置合并前不开始、合并后开始', async ({ page }) => {
  const f = state(), slug = 'dependency-parent'; flowSteps(f.root, slug); const parent = newTask(f.root, slug);
  await expect.poll(() => taskState(f.root, parent).node, { timeout: 60_000 }).toBe('await_design_approval');
  const childFake = join(f.repo, '.autome/fake/dependency-child'); mkdirSync(childFake, { recursive: true });
  writeFileSync(join(childFake, 'next'), '1');
  writeFileSync(join(childFake, '1.sh'), `while [ ! -f '${f.root}/release-dependent' ]; do '${f.root}/bin/w7-sleep' 0.1; done\n`);
  await visit(page, `/projects/${f.projectId}/tasks/new`);
  await page.getByLabel('任务正文（Markdown）').fill('# dependency-child');
  await page.getByLabel(`${parent} · ${slug}`, { exact: true }).check();
  await page.getByRole('button', { name: '创建并开始', exact: true }).click();
  const childCard = page.getByTestId('task-card').filter({ has: page.getByRole('link', { name: 'dependency-child', exact: true }) });
  await expect(childCard).toContainText(`等 ${parent} 合并`);
  const child = await childCard.getAttribute('data-task-id'); if (!child) throw Error('missing dependent');
  appendFileSync(join(f.root, 'task-ids'), child + '\n');
  expect(taskState(f.root, child).state).toBe('queued');
  cli(f.root, ['approve', parent]);
  await expect.poll(() => {
    const status = taskState(f.root, parent); expect(taskState(f.root, child).state).toBe('queued'); return status.node;
  }, { timeout: 60_000 }).toBe('await_merge');
  cli(f.root, ['merge', parent]);
  await expect.poll(() => {
    // Read the child first: the parent may finish between these CLI snapshots.
    const dependent = taskState(f.root, child);
    const status = taskState(f.root, parent);
    if (dependent.state === 'active') expect(status.state).toBe('done');
    return status.state;
  }, { timeout: 60_000 }).toBe('done');
  await expect.poll(() => taskState(f.root, child).state).toBe('active');
  // active is persisted before start_session inserts the first session (BL-13).
  // Wait for that separate observable effect without weakening the assertion.
  await expect.poll(() => {
    const detail = JSON.parse(cli(f.root, ['rpc', 'task.get', JSON.stringify({task_id: child})]));
    return detail.sessions.length;
  }).toBeGreaterThan(0);
  await expect(childCard).not.toContainText(`等 ${parent} 合并`);
});

test('B-4 命令行排队与依赖五秒内出现', async ({ page }) => {
  const f = liveState(); expect(f.tickIntervalMs).toBe(3000);
  const p = heldProject(f.root, 'queue-live'), held = p.create('live-holder');
  await expect.poll(() => taskState(f.root, held).state).toBe('active');
  const q1 = p.create('live-first'), q2 = p.create('live-second');
  let loads = 0; page.on('load', () => loads++);
  await page.goto(`${f.url}/projects/${p.projectId}`);
  const card = page.locator(`[data-task-id="${q2}"]`);
  await expect(card).toContainText('排队第 2 位');
  cli(f.root, ['queue', 'move', q2, '--top']); let start = performance.now();
  await expect(card).toContainText('排队第 1 位', { timeout: 5000 }); expect(performance.now() - start).toBeLessThanOrEqual(5000);
  cli(f.root, ['after', q2, q1]); start = performance.now();
  await expect(card).toContainText(`等 ${q1} 合并`, { timeout: 5000 }); expect(performance.now() - start).toBeLessThanOrEqual(5000);
  expect(loads).toBe(1);
  await card.getByRole('link').click();
  await page.getByRole('link', {name:'备注与依赖',exact:true}).click();
  await page.getByRole('button', {name:'清空依赖',exact:true}).click();
  await expect(page.getByRole('region', {name:'依赖',exact:true})).toContainText('无依赖');
  await page.getByLabel(`${q1} · live-first`, {exact:true}).check();
  await page.getByRole('button', {name:'保存依赖',exact:true}).click();
  await expect.poll(() => JSON.parse(cli(f.root, ['rpc','task.get',JSON.stringify({task_id:q2})])).task.after).toEqual([q1]);
  cli(f.root, ['cancel', q2]); cli(f.root, ['cancel', q1]); cli(f.root, ['cancel', held]);
});

test('BL-01 前置状态变化五秒内反映在依赖任务详情页', async ({ page }) => {
  const f = liveState(); expect(f.tickIntervalMs).toBe(3000);
  const slug = 'related-parent'; flowSteps(f.root, slug);
  const implement = join(f.repo, '.autome/fake', slug, '5.sh');
  writeFileSync(implement, `while [ ! -f '${f.root}/release-related-parent' ]; do '${f.root}/bin/w7-sleep' 0.1; done\n` + readFileSync(implement, 'utf8'));
  const parent = newTask(f.root, slug);
  await expect.poll(() => taskState(f.root, parent).node, { timeout: 60_000 }).toBe('await_design_approval');
  const child = newTask(f.root, 'related-child', ['--after', parent]);
  expect(taskState(f.root, child).state).toBe('queued');
  // Isolate TaskPage's matcher from the root loader. A root-only single-fetch
  // refresh can otherwise carry fresh task data even when related matching is
  // broken. Stub only the root route export, leaving the real detail matcher,
  // effect, event channel and loaders intact. Fail closed if bundling changes.
  let isolatedRoot = false;
  await page.route('**/assets/root-*.js', async route => {
    const response = await route.fetch();
    const source = await response.text();
    const exports = source.match(/\b[$\w]+ as shouldRevalidate\b/g);
    expect(exports).toHaveLength(1);
    await route.fulfill({ response, body: source.replace(exports![0], '__testRootRevalidate as shouldRevalidate')
      + '\nconst __testRootRevalidate = () => false;\n' });
    isolatedRoot = true;
  });
  // Isolate the real parent's event channel: reload/env notifications and child
  // events must not refresh the detail on behalf of the related-subject matcher.
  // Root also matches task.*; Router may omit _routes when both loaders match.
  let parentEvents = 0;
  await page.routeWebSocket('**/ws', socket => {
    const server = socket.connectToServer();
    server.onMessage(message => {
      const frame = JSON.parse(String(message)) as { type: string; subject?: string; core?: string; status?: string };
      if (frame.type === 'hello' || (frame.type === 'core' && frame.status === 'connected')) socket.send(message);
      else if (frame.type === 'event' && frame.subject === parent) {
        parentEvents++;
        socket.send(message);
      }
    });
  });
  let loads = 0; page.on('load', () => loads++);
  await page.goto(`${f.url}/tasks/${child}?tab=notes`);
  await expect(page.getByRole('button', { name: '提交备注', exact: true })).toBeEnabled();
  expect(isolatedRoot).toBe(true);
  const dependency = page.getByRole('region', { name: '依赖', exact: true }).locator('li').filter({ hasText: parent });
  await expect(dependency).toContainText('等你批准');
  parentEvents = 0;
  cli(f.root, ['approve', parent]); const start = performance.now();
  await expect(dependency).not.toContainText('等你批准', { timeout: 5000 });
  await timedExpect(test.info(), 'controls 动态断言', Math.max(1, 5000 - (performance.now() - start)), timeout => expect(dependency).toContainText('实现', { timeout }));
  expect(parentEvents).toBeGreaterThan(0);
  expect(performance.now() - start).toBeLessThanOrEqual(5000);
  expect(taskState(f.root, child).state).toBe('queued');
  expect(loads).toBe(1);
  await page.screenshot({ path: join(checkout, '.autome/output/m10-dependency.png') });
  cli(f.root, ['cancel', child]); cli(f.root, ['cancel', parent]);
});

test('E-1 页面改全局角色与项目覆盖后 config.get 读回并能恢复', async ({ page }) => {
  const f=state();
  const rpc=(method:string,params:object)=>JSON.parse(cli(f.root,['rpc',method,JSON.stringify(params)]));
  const original=rpc('config.get',{}).global.roles.plan;
  const projectOriginal=readFileSync(join(f.repo,'.autome/config.toml'),'utf8');
  try {
    rpc('config.set_role',{role:'plan',effort:'xhigh'});
    await visit(page,'/loop');
    await expect(page.getByTestId('loop-role')).toHaveCount(6);
    await page.locator('[data-role=plan]').getByRole('button').click();
    await expect(page.getByText('其它：xhigh',{exact:true})).toBeVisible();
    await page.getByLabel('模型',{exact:true}).fill('m07-browser-global');
    await page.getByRole('radiogroup',{name:'Effort',exact:true}).getByText('low',{exact:true}).click();
    await page.getByRole('button',{name:'保存',exact:true}).click();
    await expect.poll(()=>rpc('config.get',{}).global.roles.plan.model).toBe('m07-browser-global');
    await page.getByRole('button',{name:'关闭',exact:true}).click();
    await visit(page,`/loop?project=${f.projectId}`);
    await page.locator('[data-role=plan]').getByRole('button').click();
    await page.getByLabel('模型',{exact:true}).fill('m07-browser-project');
    await page.getByRole('button',{name:'保存',exact:true}).click();
    await expect.poll(()=>rpc('config.get',{project_id:f.projectId}).resolved.roles[0].config.model).toBe('m07-browser-project');
    expect(rpc('config.get',{}).global.roles.plan.model).toBe('m07-browser-global');
    page.once('dialog',dialog=>{expect(dialog.message()).toContain('项目覆盖会一起清掉');void dialog.accept();});
    await page.getByRole('button',{name:'恢复跟随全局',exact:true}).click();
    await expect.poll(()=>rpc('config.get',{project_id:f.projectId}).resolved.roles[0].provenance).toBe('global');
    await page.getByRole('button',{name:'关闭',exact:true}).click();
    await page.screenshot({path:join(checkout,'.autome/output/m07-loop.png')});
  } finally {
    rpc('config.set_role',{role:'plan',...original});
    writeFileSync(join(f.repo,'.autome/config.toml'),projectOriginal);
  }
});
