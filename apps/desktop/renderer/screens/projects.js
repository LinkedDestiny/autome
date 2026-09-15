// 我的项目 — requirements P-07 / U-03.
//
// One tile per registered project, carrying exactly what P-07 asks for: name,
// directory, default branch, how many tasks are running, how many are waiting
// on the user, and an abnormal marker. A project whose onboarding never
// finished says so and offers to continue it (C-02).
//
// Adding a project is the one place a filesystem path enters the system, and
// the renderer does not get to name it: `write.pickProject()` asks Main to
// open the dialog and returns whatever Main chose (technical design §15).

import {
  h, icon, text, reveal, tag, spinnerTag, empty, activateOnKey,
} from '../lib/dom.js';
import { read, attempt, registerWrite } from '../lib/api.js';
import { openModal, closeButton, close } from '../lib/overlay.js';

export const id = 'projects';
export const nav = 'projects';

// The design's app-tile palette, cycled by position so a project keeps the
// same colour for as long as the list order is stable.
const PALETTE = ['blue', 'teal', 'orange', 'pink', 'purple', 'green', 'yellow', 'brown'];

export async function load() {
  return read('listProjects');
}

export function render(host, data, ctx) {
  const screen = h('div.screen.active', { 'data-screen': 'projects' });
  const projects = (data && data.projects) || [];

  screen.appendChild(
    h('div.pagehead', [
      h('h1.ribbon.ribbon--blue', [h('span.ribbon__front', { text: '我的项目' })]),
      h('span.pagehead__sub', { text: '项目 = 一个目录 · 不是 Git 仓库会自动 init · 任务都在项目下' }),
      h('div.pagehead__actions', [
        registerWrite(
          h('button.btn.btn--primary', { type: 'button', onClick: () => addProject(ctx) }, [
            icon('plus'),
            text('添加项目'),
          ])
        ),
      ]),
    ])
  );

  const grid = h('div.grid.grid--projects');
  projects.forEach((entry, index) => grid.appendChild(reveal(projectTile(entry, index, ctx), index + 1)));
  grid.appendChild(reveal(addTile(ctx), projects.length + 1));
  screen.appendChild(grid);

  if (!projects.length) {
    screen.appendChild(empty('还没有项目。选一个目录，Autome 会把它变成可以接任务的仓库。'));
  }

  host.appendChild(screen);
}

function projectTile(entry, index, ctx) {
  const project = entry.project || {};
  const counts = entry.counts || {};
  const colour = PALETTE[index % PALETTE.length];
  const onboarding = project.onboarding || {};
  const pending = onboarding.onboarding === 'in_progress';

  const tile = h(`div.card.card--pattern.card--pattern-${colour}.card--hoverable.ptile`, {
    tabindex: '0',
    role: 'button',
    onClick: () =>
      pending ? openOnboarding(project, ctx) : ctx.navigate('project', { projectId: project.id }),
    onKeyDown: activateOnKey,
  });

  tile.appendChild(
    h('div.ptile__top', [
      h('span.ptile__name', { text: project.display_name || project.id }),
      h('span.ptile__kind', { text: project.default_branch || '—' }),
    ])
  );

  const slots = `${entry.slots_in_use || 0} / ${project.parallel_limit || 0}`;
  tile.appendChild(
    h('div.ptile__intent', {
      text: `${project.path || ''} · 并行 ${slots}${counts.queued ? ` · 排队 ${counts.queued}` : ''}`,
    })
  );

  const foot = h('div.ptile__foot');
  if (counts.running) foot.appendChild(spinnerTag(`${counts.running} 运行中`, 'solid-teal'));
  if (counts.awaiting_user) foot.appendChild(tag(`${counts.awaiting_user} 等待我`, 'solid-yellow'));
  // P-07's 异常标记. A failed task is the only abnormal state the list knows
  // about; it is red because the task is stopped, not merely slow.
  if (counts.failed) foot.appendChild(tag(`${counts.failed} 失败`, 'solid-red', 'warn'));
  if (counts.done) foot.appendChild(tag(`${counts.done} 已完成`, 'outlined'));
  if (pending) {
    foot.appendChild(tag(`Onboarding 第 ${onboarding.step} / 5 步`, 'dashed-brown'));
  }
  if (!foot.childNodes.length) foot.appendChild(tag('还没有任务', 'outlined'));

  const cta = h('span.ptile__cta', { text: pending ? '继续 ' : '进入 ' });
  cta.appendChild(icon('arrow', 'ic--sm'));
  foot.appendChild(cta);
  tile.appendChild(foot);
  return tile;
}

function addTile(ctx) {
  const tile = h('div.card.card--dashed.card--hoverable.ptile.ptile--new', {
    tabindex: '0',
    role: 'button',
    onClick: () => addProject(ctx),
    onKeyDown: activateOnKey,
  });
  tile.appendChild(h('span.big', [icon('plus', 'ic--lg')]));
  const label = h('div', [h('b', { text: '添加项目' }), h('br')]);
  label.appendChild(h('span.small', { text: '选一个目录，剩下的自动完成' }));
  tile.appendChild(label);
  registerWrite(tile);
  return tile;
}

/**
 * P-01/P-02. There is no "confirm trust" step: choosing the directory is the
 * consent. Main runs the dialog, the core does the init, and a cancelled
 * dialog is not an error — it says `{ cancelled: true }` and we say nothing.
 */
export async function addProject(ctx) {
  await attempt({
    label: '添加项目',
    success: false,
    run: (write) => write.pickProject(),
    onDone: async (result) => {
      if (!result || result.cancelled) return;
      const project = result.project || result;
      if (project && project.id) ctx.navigate('project', { projectId: project.id });
      else await ctx.refresh();
    },
  });
}

/**
 * C-02's wizard, as far as this screen is concerned: the five steps are run by
 * the core in a visible terminal, so the modal states where it got to and
 * offers the two things the user can decide — carry on, or skip the rest.
 */
function openOnboarding(project, ctx) {
  const step = (project.onboarding && project.onboarding.step) || 3;
  const steps = [
    ['选择目录', project.path || ''],
    ['初始化 .autome', '脚手架已写入 · init commit 已提交'],
    ['Claude Code 起草画像与 AGENTS.md', '在 iTerm2 可见终端中运行'],
    ['确认产物', 'docs/agent-project-profile.md 与 AGENTS.md'],
    ['Loop 配置', '从全局默认复制一份，按这个项目改'],
  ];

  const body = steps.map(([title, detail], index) => {
    const n = index + 1;
    const cls = n < step ? 'wstep done' : n === step ? 'wstep now' : 'wstep';
    const marker = h('div.wstep__n');
    if (n < step) marker.appendChild(icon('check', 'ic--sm'));
    else marker.appendChild(text(String(n)));
    return h(`div.${cls.split(' ').join('.')}`, [
      marker,
      h('div', [h('div.wstep__t', { text: title }), h('div.wstep__d', { text: detail })]),
    ]);
  });

  openModal({
    title: `Onboarding · ${project.display_name || project.id}`,
    lead: '五步，随时可以跳过剩下的。',
    body,
    footer: [
      closeButton('稍后继续'),
      registerWrite(
        h('button.modal__btn', {
          type: 'button',
          onClick: () =>
            attempt({
              label: '已跳过 Onboarding',
              success: '使用全局默认配置；首个任务会自己补画像。',
              run: (write) => write.skipOnboarding(project.id),
              onDone: () => {
                close();
                return ctx.refresh();
              },
            }),
        }, [text('跳过剩下的')])
      ),
      registerWrite(
        h('button.btn.btn--yellow.ml-auto', {
          type: 'button',
          onClick: () =>
            attempt({
              label: '继续 Onboarding',
              success: `已推进到第 ${Math.min(step + 1, 5)} 步。`,
              run: (write) => write.advanceOnboarding(project.id),
              onDone: () => {
                close();
                return ctx.refresh();
              },
            }),
        }, [text('继续下一步')])
      ),
    ],
  });
}
