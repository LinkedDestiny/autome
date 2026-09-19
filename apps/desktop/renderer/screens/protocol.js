// 协议版本 — plan §6.5 and §6.6.
//
// The page exists to put evidence in one place, and it deliberately stops
// there. There is no automatic acceptance, no automatic rollback, and no
// ratchet: this system produces single digits of tasks a month across projects
// of wildly different difficulty, and a ratchet needs samples. What it can do
// honestly is show what each version's tasks cost, what each change predicted,
// and what happened — and then get out of the way.
//
// Three rules it holds to, all of them about not overclaiming:
//
//  - Fewer than three tasks says "样本不足" instead of drawing a line between
//    two points.
//  - Comparisons are within one project. Cross-project rows are listed and
//    never differenced.
//  - Rolling back is a *forward* commit. The button says so, because a user
//    who thinks they moved a tag will be surprised by the version number.

import { h, icon, text, tag, cardHead, sectionHead, empty, repoList } from '../lib/dom.js';
import { read, readOr, attempt, registerWrite } from '../lib/api.js';
import * as labels from '../lib/labels.js';

export const id = 'protocol';
export const nav = 'settings';

export async function load(ctx) {
  const projectId = ctx.params.projectId || null;
  const protocol = await read('protocol');
  const gate = await readOr({ initialised: false }, 'protocolEval');
  const triggers = await readOr({ triggers: [], suggest: false }, 'protocolTriggers');
  const versions = projectId
    ? await readOr({ rows: [] }, 'protocolVersions', projectId)
    : { rows: [], metric_names: [], changelog_by_tag: {} };
  let projectName = null;
  if (projectId) {
    const list = await readOr({ projects: [] }, 'listProjects');
    const entry = (list.projects || []).find((p) => p.project && p.project.id === projectId);
    projectName = entry ? entry.project.display_name : projectId;
  }
  return { protocol, gate, triggers, versions, projectId, projectName };
}

export function render(host, data, ctx) {
  const screen = h('div.screen.active', { 'data-screen': 'protocol' });
  const p = data.protocol || {};

  screen.appendChild(
    h('div.crumb', [
      h('button', { type: 'button', onClick: () => ctx.navigate('settings') }, [
        text('全局设置'),
      ]),
      h('span.sep', { text: '›' }),
      h('b', { text: '协议版本' }),
    ])
  );

  screen.appendChild(
    h('div.pagehead.pagehead--tight', [
      h('h1.ribbon.ribbon--blue', [h('span.ribbon__front', { text: '协议版本' })]),
      h('span.pagehead__sub', {
        text: p.initialised
          ? `${p.path} · 当前 ${labels.protocolRef((p.current || {}).wire)}`
          : '还没有协议仓库——添加第一个项目时会建好',
      }),
      h('div.pagehead__actions', [improveButton(data, ctx)]),
    ])
  );

  if (!p.initialised) {
    screen.appendChild(empty('协议仓库会在你添加第一个项目时建好，内容来自 Autome 自带的种子。'));
    host.appendChild(screen);
    return;
  }

  screen.appendChild(
    h('div.cardgrid.cardgrid--3', [currentCard(p), gateCard(data.gate), triggersCard(data, ctx)])
  );

  screen.appendChild(
    sectionHead(
      '每一版的表现',
      data.projectId
        ? `${data.projectName} · 少于 3 个任务的版本只列数字，不下结论`
        : '从项目页进来才能看到某个项目下各版本的指标——跨项目的任务难度不可比，平均到一起得到的数字什么都不是'
    )
  );
  screen.appendChild(versionTable(data));

  screen.appendChild(sectionHead('改动与它们的预测', '每条改动预测了一个指标，结果由 Autome 回填'));
  screen.appendChild(changelog(data));

  host.appendChild(screen);
}

function improveButton(data, ctx) {
  const suggest = (data.triggers || {}).suggest;
  const button = registerWrite(
    h(
      `button.btn${suggest ? '.btn--primary' : ''}`,
      {
        type: 'button',
        onClick: () =>
          attempt({
            label: '改进 Loop',
            success: '协议仓库上开了一个新任务',
            run: (write) => write.improveProtocol(),
            onDone: () => ctx.refresh(),
          }),
      },
      [icon('plus'), text('改进 Loop')]
    )
  );
  if (!suggest) {
    button.title = '现在也可以开，只是还没有攒够证据';
  }
  return button;
}

function currentCard(p) {
  const current = p.current || {};
  const rows = [
    ['当前版本', current.tag || '—'],
    ['内容哈希', (current.hash || '').slice(0, 16) || '—'],
    ['正文字节', `${p.bytes ?? '—'} / ${p.byte_budget ?? '—'}`],
    ['已发布', `${(p.tags || []).length} 版`],
  ];
  const card = h('div.card', [cardHead('当前'), repoList(rows)]);
  if (p.working_tree_differs) {
    // Not an error: editing the repository directly is allowed. But "I changed
    // the protocol and nothing happened" needs an explanation, and this is it.
    card.appendChild(
      h('div.quiet', {
        text: '工作区和最新标签不一致。已经在跑的任务不受影响——它们各自持有一份副本——新任务用的也还是最新标签那一版，直到你发布。',
      })
    );
  }
  if ((p.contract_breaches || []).length) {
    card.appendChild(
      h('div.alertbar', [
        text(`契约区被改动了 ${p.contract_breaches.length} 处，这一版不能用`),
      ])
    );
    for (const b of p.contract_breaches) card.appendChild(h('div.quiet', { text: b }));
  }
  return card;
}

function gateCard(gate) {
  if (!gate || !gate.initialised) {
    return h('div.card', [cardHead('eval 门'), empty('还没有协议仓库')]);
  }
  const head = cardHead(
    'eval 门',
    gate.ok ? tag('通过', 'soft-green') : tag(`${gate.failures.length} 项不通过`, 'soft-red')
  );
  const card = h('div.card', [head]);
  card.appendChild(
    h('div.quiet', {
      text: '静态层与示例层，对着协议仓库的工作区跑。行为层要真跑 CLI，只在元任务的审计轮跑。',
    })
  );
  for (const f of gate.failures || []) {
    card.appendChild(h('div.quiet', { text: `✗ [${f.layer}] ${f.detail}` }));
  }
  for (const w of gate.warnings || []) {
    card.appendChild(h('div.quiet', { text: `! [${w.layer}] ${w.detail}` }));
  }
  if (gate.ok && !(gate.warnings || []).length) {
    card.appendChild(h('div.quiet', { text: `${(gate.passed || []).length} 项检查全部通过` }));
  }
  return card;
}

function triggersCard(data, ctx) {
  const t = data.triggers || {};
  const card = h('div.card', [
    cardHead('要不要改协议', t.suggest ? tag('有证据了', 'soft-yellow') : null),
  ]);
  if (!(t.triggers || []).length) {
    card.appendChild(
      h('div.quiet', {
        text: '还没有攒够证据。要么是完成的任务还不够多，要么是还没有同一条教训在两个任务里重复出现。',
      })
    );
  } else {
    for (const reason of t.triggers) card.appendChild(h('div.quiet', { text: `· ${reason}` }));
    card.appendChild(
      h('div.quiet', {
        text: '这些只是提示。一次协议迭代要花掉一整个 Loop 的会话，开不开由你。',
      })
    );
  }
  const _ = ctx;
  return card;
}

function versionTable(data) {
  const rows = (data.versions || {}).rows || [];
  if (!rows.length) {
    return empty(
      data.projectId
        ? '这个项目还没有已终结并记录了指标的任务。'
        : '从某个项目页进来，才能看到它各版本的指标。'
    );
  }
  const metrics = (data.versions || {}).metric_names || [];
  const table = h('table.metrics');
  const head = h('tr', [h('th', { text: '版本' }), h('th', { text: '任务数' })]);
  for (const m of metrics) head.appendChild(h('th', { text: labels.metricLabel(m) }));
  table.appendChild(h('thead', [head]));

  const body = h('tbody');
  for (const row of rows) {
    const tr = h('tr');
    const name = h('td', [text(row.tag)]);
    if ((row.warnings || []).length) {
      // Red is a warning, not a verdict. The version page decides nothing.
      name.appendChild(tag(`${row.warnings.length} 项变差`, 'soft-red'));
      name.title = row.warnings.map(labels.metricLabel).join('、');
    }
    tr.appendChild(name);
    tr.appendChild(
      h('td', { text: row.enough_samples ? String(row.samples) : `${row.samples} · 样本不足` })
    );
    for (const m of metrics) {
      tr.appendChild(h('td', { text: labels.metricValue(m, (row.means || {})[m]) }));
    }
    body.appendChild(tr);
  }
  table.appendChild(body);
  return table;
}

function changelog(data) {
  const log = (data.protocol || {}).changelog;
  const versions = (log && log.versions) || [];
  if (!versions.length) return empty('CHANGELOG 里还没有任何改动。');

  const wrap = h('div');
  for (const version of versions) {
    wrap.appendChild(sectionHead(version.tag, `${version.entries.length} 条改动`, rollbackRow(version, data)));
    for (const entry of version.entries) wrap.appendChild(entryCard(entry));
  }
  return wrap;
}

function rollbackRow(version, data) {
  const current = ((data.protocol || {}).current || {}).tag;
  if (version.tag === current || version.tag === '未发布') return null;
  return [
    registerWrite(
      h('button.btn.btn--sm', {
        type: 'button',
        // Deliberately explicit about what this does. A user expecting the tag
        // to move will be surprised by the new version number, and surprise
        // about version numbers is how a metrics table stops being trusted.
        title: '生成一个内容与该版本相同的新版本，不移动标签',
        onClick: () =>
          attempt({
            label: `回到 ${version.tag} 的内容`,
            success: '已生成一个内容相同的新版本',
            run: (write) => write.rollbackProtocol(version.tag),
          }),
      }, [text('回到这一版的内容')])
    ),
  ];
}

function entryCard(entry) {
  const predicted = entry.predicted_impact || {};
  const realized = entry.realized_impact;
  const card = h('div.card', [
    cardHead(`${entry.id} · ${entry.kind}`, verdictTag(entry, realized)),
  ]);
  card.appendChild(h('div.quiet', { text: entry.clause }));
  card.appendChild(
    h('div.quiet', {
      text: `预测：${labels.metricLabel(predicted.metric)} ${directionText(predicted.direction)}，${predicted.horizon} 个任务之内（${predicted.scope === 'project' ? '按项目' : '按任务'}）`,
    })
  );
  if (realized) {
    card.appendChild(
      h('div.quiet', {
        text: `实际：${labels.metricValue(realized.metric, realized.before)} → ${labels.metricValue(realized.metric, realized.after)}（前 ${realized.samples_before} 个任务 / 后 ${realized.samples_after} 个）`,
      })
    );
  } else {
    card.appendChild(h('div.quiet', { text: '实际：还没到期，或者样本不足' }));
  }
  if ((entry.evidence || []).length) {
    card.appendChild(h('div.quiet', { text: `依据：${entry.evidence.join('、')}` }));
  }
  if (entry.eval) card.appendChild(h('div.quiet', { text: `用例：${entry.eval}` }));
  return card;
}

function verdictTag(entry, realized) {
  if (!realized) return tag('等结果', 'outlined');
  const thin = realized.samples_before < 3 || realized.samples_after < 3;
  if (thin) return tag('样本不足', 'outlined');
  const down = realized.after < realized.before;
  const flat = realized.after <= realized.before;
  const direction = (entry.predicted_impact || {}).direction;
  const held = direction === 'down' ? down : direction === 'up' ? realized.after > realized.before : flat;
  return held ? tag('符合预测', 'soft-green') : tag('与预测相反', 'soft-red');
}

function directionText(direction) {
  if (direction === 'down') return '会降';
  if (direction === 'up') return '会升';
  return '不会变差';
}
