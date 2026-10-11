// 协议版本 — plan §6.5 and §6.6.
//
// The protocol ships with Autome: it is compiled into the core from the
// autome-v2 source, and changing it means editing that source and redeploying.
// There is no protocol repository, no pinning and no rollback on this machine.
// What the page can still do honestly is show which version is built in, what
// each version's tasks cost, and what each change predicted — and then get out
// of the way.
//
// Two rules it holds to, both about not overclaiming:
//
//  - Fewer than three tasks says "样本不足" instead of drawing a line between
//    two points.
//  - Comparisons are within one project. Cross-project rows are listed and
//    never differenced.

import { h, text, tag, cardHead, sectionHead, empty, repoList } from '../lib/dom.js';
import { read, readOr } from '../lib/api.js';
import * as labels from '../lib/labels.js';

export const id = 'protocol';
export const nav = 'settings';

export async function load(ctx) {
  const projectId = ctx.params.projectId || null;
  const protocol = await read('protocol');
  const versions = projectId
    ? await readOr({ rows: [] }, 'protocolVersions', projectId)
    : { rows: [], metric_names: [], changelog_by_tag: {} };
  let projectName = null;
  if (projectId) {
    const list = await readOr({ projects: [] }, 'listProjects');
    const entry = (list.projects || []).find((p) => p.project && p.project.id === projectId);
    projectName = entry ? entry.project.display_name : projectId;
  }
  return { protocol, versions, projectId, projectName };
}

export function render(host, data, ctx) {
  const screen = h('div.screen.active', { 'data-screen': 'protocol' });
  const p = data.protocol || {};

  // The crumb names where this page was opened from, not where it usually
  // lives. It is reachable two ways — 全局设置, and a project's 指标 section
  // via 按版本看 — and a crumb that always said 全局设置 stranded the second
  // one: the project it was showing versions *for* had no way back.
  screen.appendChild(
    h('div.crumb', [
      data.projectId
        ? h('button', {
            type: 'button',
            onClick: () => ctx.navigate('project', { projectId: data.projectId }),
          }, [text(data.projectName || '项目')])
        : h('button', { type: 'button', onClick: () => ctx.navigate('settings') }, [
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
        text: `随 Autome 内置 · 当前 ${labels.protocolRef((p.current || {}).wire)}`,
      }),
    ])
  );

  screen.appendChild(h('div.cardgrid.cardgrid--2', [currentCard(p), sourceCard()]));

  screen.appendChild(
    sectionHead(
      '每一版的表现',
      data.projectId
        ? `${data.projectName} · 少于 3 个任务的版本只列数字，不下结论`
        : '从项目页进来才能看到某个项目下各版本的指标——跨项目的任务难度不可比，平均到一起得到的数字什么都不是'
    )
  );
  screen.appendChild(versionTable(data));

  screen.appendChild(sectionHead('改动与它们的预测', '每条改动预测了一个指标'));
  screen.appendChild(changelog(data));

  host.appendChild(screen);
}

function currentCard(p) {
  const current = p.current || {};
  const rows = [
    ['当前版本', current.tag || '—'],
    ['内容哈希', (current.hash || '').slice(0, 16) || '—'],
    ['正文字节', `${p.bytes ?? '—'} / ${p.byte_budget ?? '—'}`],
  ];
  return h('div.card', [cardHead('当前', tag('内置', 'soft-green', 'check')), repoList(rows)]);
}

/// Where the rules come from, and the one way to change them.
function sourceCard() {
  return h('div.card', [
    cardHead('怎么改'),
    h('div.quiet', {
      text: '协议随 Autome 一起发布。要改它，在 autome-v2 源码里改 protocol/，跑过 eval 后重新部署。',
    }),
    h('div.quiet.mt-8', {
      text: '任务创建时固定一份副本，之后换了新版本也影响不到已经在跑的任务。',
    }),
  ]);
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
    wrap.appendChild(sectionHead(version.tag, `${version.entries.length} 条改动`));
    for (const entry of version.entries) wrap.appendChild(entryCard(entry));
  }
  return wrap;
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
