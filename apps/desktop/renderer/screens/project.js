// 项目详情 — requirements U-04 / section 4.3, plus the 新建任务 drawer (U-05).
//
// Three information cards (repo, Loop config, rules and skills), then the task
// list split into 未完成 and 已完成 exactly as T-12 requires, with 归档 on the
// completed cards and an 已归档 drawer that can put one back.
//
// The split is computed from the task's own `state` and `archived` flag rather
// than from a separate list, so a task cannot appear in two sections because
// two queries disagreed.

import {
  h, icon, text, reveal, tag, spinnerTag, progress, repoList, cardHead, sectionHead,
  empty, activateOnKey,
} from '../lib/dom.js';
import { read, readOr, attempt, registerWrite } from '../lib/api.js';
import { openDrawer, closeButton, close } from '../lib/overlay.js';
import * as labels from '../lib/labels.js';
import { openMergeModal } from './task.js';

export const id = 'project';
export const nav = 'projects';

export async function load(ctx) {
  const projectId = ctx.params.projectId;
  const project = await read('getProject', projectId);
  // The skills inventory is a second call because the core keeps the two
  // concerns apart; a failure here must not blank the whole project page, so
  // it degrades to "not listed" rather than to an error screen.
  const skills = await readOr({ skills: [] }, 'listSkills', projectId);
  return { ...project, skills: skills.skills || [] };
}

export function render(host, data, ctx) {
  const screen = h('div.screen.active', { 'data-screen': 'project' });
  const project = (data && data.project) || {};
  const tasks = (data && data.tasks) || [];
  const counts = (data && data.counts) || {};

  const active = tasks.filter((t) => !t.archived && !isFinished(t));
  const doneTasks = tasks.filter((t) => !t.archived && t.state && t.state.state === 'done');
  const archived = tasks.filter((t) => t.archived);
  const slotsInUse = active.filter((t) => t.state && t.state.state === 'active').length;

  screen.appendChild(
    h('div.crumb', [
      h('button', { type: 'button', onClick: () => ctx.navigate('projects') }, [text('项目')]),
      h('span.sep', { text: '›' }),
      h('b', { text: project.display_name || project.id || '' }),
    ])
  );

  screen.appendChild(
    h('div.pagehead.pagehead--tight', [
      h('h1.ribbon.ribbon--blue', [
        h('span.ribbon__front', { text: project.display_name || project.id || '项目' }),
      ]),
      h('span.pagehead__sub', {
        text: `${project.path || ''} · 默认分支 ${project.default_branch || '—'} · 并行 ${slotsInUse} / ${project.parallel_limit || 0}${counts.queued ? ` · 排队 ${counts.queued}` : ''}`,
      }),
      h('div.pagehead__actions', [
        registerWrite(
          h('button.btn', {
            type: 'button',
            onClick: () =>
              attempt({
                label: '打开目录',
                success: project.path,
                run: (write) => write.open({ kind: 'project', projectId: project.id }),
              }),
          }, [text('打开目录')])
        ),
        registerWrite(
          h('button.btn.btn--primary', {
            type: 'button',
            onClick: () => openNewTaskDrawer(data, ctx),
          }, [icon('plus'), text('新建任务')])
        ),
      ]),
    ])
  );

  const cards = h('div.cardgrid.cardgrid--3', [
    reveal(repoCard(project, data), 1),
    reveal(configCard(data, ctx), 2),
    reveal(rulesCard(data, ctx), 3),
  ]);
  screen.appendChild(cards);

  // ---- 未完成 ----------------------------------------------------------
  screen.appendChild(
    sectionHead(
      '未完成',
      `${active.length} 个 · ${counts.running || 0} 运行中 · ${counts.queued || 0} 排队 · ${counts.awaiting_user || 0} 等待我`
    )
  );
  const activeGrid = h('div.cardgrid.cardgrid--4');
  active.forEach((task, index) => activeGrid.appendChild(reveal(taskCard(task, ctx), index + 4)));
  activeGrid.appendChild(reveal(newTaskTile(data, ctx), active.length + 4));
  screen.appendChild(activeGrid);

  // ---- 已完成 ----------------------------------------------------------
  const doneRight = [
    h('button.btn.btn--sm.btn--text', {
      type: 'button',
      onClick: () => openArchiveDrawer(archived, ctx),
    }, [text(`已归档 ${archived.length}`)]),
  ];
  screen.appendChild(
    sectionHead(
      '已完成',
      doneTasks.length
        ? `${doneTasks.length} 个 · 已合并进 ${project.default_branch || 'main'} · 归档后移出列表，可恢复`
        : '还没有已完成的任务',
      doneRight
    )
  );
  if (doneTasks.length) {
    const doneGrid = h('div.cardgrid.cardgrid--4');
    doneTasks.forEach((task, index) =>
      doneGrid.appendChild(reveal(doneCard(task, ctx), index + 1))
    );
    screen.appendChild(doneGrid);
  }

  host.appendChild(screen);
}

function isFinished(task) {
  const state = task.state && task.state.state;
  return state === 'done' || state === 'cancelled';
}

// ---------------------------------------------------------------------------
// The three information cards
// ---------------------------------------------------------------------------

function repoCard(project, data) {
  const onboarding = project.onboarding || {};
  const onboardingText =
    onboarding.onboarding === 'in_progress'
      ? `进行中 · 第 ${onboarding.step} / 5 步`
      : onboarding.onboarding === 'skipped'
        ? '已跳过 · 使用全局默认'
        : '已完成';
  const violations = (data && data.violations) || [];
  const status = violations.length
    ? tag(`${violations.length} 项配置被阻止`, 'soft-red', 'warn')
    : tag('就绪', 'soft-green', 'check');

  const card = h('div.card.card--pattern.card--pattern-blue.card--pad.col', [
    cardHead('仓库', status),
    repoList(
      [
        ['目录', project.path || '—'],
        ['默认分支', project.default_branch || '—'],
        ['并行上限', String(project.parallel_limit || 0)],
        ['Onboarding', onboardingText],
      ],
      'inherit'
    ),
  ]);
  return card;
}

function configCard(data, ctx) {
  const config = (data && data.config) || {};
  const roles = config.roles || [];
  const overridden = roles.filter((r) => r.provenance === 'project').length;
  const project = (data && data.project) || {};

  const card = h('div.card.card--pad.col.card--link', {
    tabindex: '0',
    role: 'button',
    onClick: () => ctx.navigate('routing', { projectId: project.id }),
    onKeyDown: activateOnKey,
  });
  card.appendChild(
    cardHead('Loop 配置', tag(overridden ? `${overridden} 项覆盖` : '全部跟随全局', 'soft-brown'))
  );

  const lines = h('div.roleline');
  for (const resolved of roles) {
    lines.appendChild(h('b', { text: labels.roleLabel(resolved.role) }));
    const value = h('span', { text: labels.runtimeSummary(resolved.config) });
    if (!resolved.config.enabled) value.appendChild(tag('已关闭', 'outlined'));
    if (resolved.provenance === 'project') value.appendChild(h('span.srcchip.srcchip--p', { text: '项目' }));
    lines.appendChild(value);
  }
  card.appendChild(lines);

  const defaults = config.loop_defaults || {};
  card.appendChild(
    h('div.quiet.mt-6', {
      text: `并行 ${defaults.parallel ?? '—'} · 设计 ${defaults.design_rounds ?? '—'} 轮 · 预算系数 ${defaults.budget_factor ?? '—'} · 改动对下一个节点生效`,
    })
  );
  const cta = h('span.card__cta', { text: '打开路由图 ' });
  cta.appendChild(icon('arrow', 'ic--sm'));
  card.appendChild(cta);
  return card;
}

function rulesCard(data, ctx) {
  const rules = (data && data.rules) || [];
  const skills = (data && data.skills) || [];
  const project = (data && data.project) || {};
  const projectSkills = skills.filter((s) => s.project_scoped).map((s) => s.name);
  const globalSkills = skills.filter((s) => !s.project_scoped).map((s) => s.name);

  const card = h('div.card.card--pad.col.card--link', {
    tabindex: '0',
    role: 'button',
    onClick: () => ctx.navigate('skills', { projectId: project.id }),
    onKeyDown: activateOnKey,
  });
  card.appendChild(cardHead('规则与技能'));
  card.appendChild(
    repoList([
      ['规则', rules.length ? rules.join(' · ') : '仓库里还没有规则文件'],
      ['项目技能', projectSkills.length ? projectSkills.join(' · ') : '无'],
      ['全局技能', globalSkills.length ? globalSkills.join(' · ') : '无'],
    ])
  );
  // C-10: Autome lists rule files, it never edits them.
  card.appendChild(h('div.quiet.mt-6', { text: '规则是仓库内文件，Autome 只列出，不在应用内编辑。' }));
  const cta = h('span.card__cta', { text: '技能盘点与绑定 ' });
  cta.appendChild(icon('arrow', 'ic--sm'));
  card.appendChild(cta);
  return card;
}

// ---------------------------------------------------------------------------
// Task cards
// ---------------------------------------------------------------------------

function taskCard(task, ctx) {
  const status = labels.taskStatus(task);
  const node = labels.stateNode(task.state);
  const flowIndex = labels.FLOW.findIndex((s) => s.key === node);

  const card = h('div.card.taskcard.card--link', {
    tabindex: '0',
    role: 'button',
    onClick: () => ctx.navigate('task', { taskId: task.id }),
    onKeyDown: activateOnKey,
  });

  const statusTag = status.spinning
    ? spinnerTag(status.label, status.variant)
    : tag(status.label, status.variant, status.icon);
  statusTag.classList.add('ml-auto');
  card.appendChild(h('div.row', [h('span.taskcard__id', { text: task.id }), statusTag]));
  card.appendChild(h('div.taskcard__t', { text: task.title || task.request || task.slug }));

  if (task.state && task.state.state === 'queued') {
    card.appendChild(h('div.quiet', { text: '等待槽位 · 任一任务释放后自动启动' }));
  } else if (task.state && task.state.state === 'failed') {
    card.appendChild(h('div.quiet', { text: labels.failureReason(task.state.reason) }));
  } else {
    card.appendChild(
      progress(
        flowIndex >= 0 ? (flowIndex + 1) / labels.FLOW.length : 0,
        flowIndex >= 0 ? `${flowIndex + 1} / ${labels.FLOW.length}` : '—',
        node === 'implement' ? null : 'blue'
      )
    );
    card.appendChild(h('div.quiet', { text: `已存在 ${labels.duration(task.created_at) || '—'}` }));
  }

  if (node === 'await_merge') {
    card.appendChild(
      h('div.btnrow', [
        registerWrite(
          h('button.btn.btn--sm.btn--primary', {
            type: 'button',
            onClick: (event) => {
              event.stopPropagation();
              openMergeModal(task.id, ctx);
            },
          }, [text('合并')])
        ),
      ])
    );
  }
  return card;
}

function doneCard(task, ctx) {
  const card = h('div.card.taskcard.taskcard--done.card--link', {
    tabindex: '0',
    role: 'button',
    onClick: () => ctx.navigate('task', { taskId: task.id }),
    onKeyDown: activateOnKey,
  });
  const doneTag = tag('已完成', 'soft-green', 'check');
  doneTag.classList.add('ml-auto');
  card.appendChild(h('div.row', [h('span.taskcard__id', { text: task.id }), doneTag]));
  card.appendChild(h('div.taskcard__t', { text: task.title || task.request || task.slug }));

  const meta = task.merge_commit
    ? `合并 ${task.merge_commit} · ${labels.day(task.completed_at)}`
    : `完成于 ${labels.day(task.completed_at)}`;
  const row = h('div.row.gap-8', [h('span.quiet', { text: meta })]);
  row.appendChild(
    registerWrite(
      h('button.btn.btn--sm.btn--text.ml-auto', {
        type: 'button',
        onClick: (event) => {
          event.stopPropagation();
          archiveTask(task, ctx);
        },
      }, [text('归档')])
    )
  );
  card.appendChild(row);
  return card;
}

function archiveTask(task, ctx) {
  return attempt({
    label: `已归档 ${task.id}`,
    success: '任务目录移入 docs/.archive/，从列表移出；可在「已归档」里恢复。',
    run: (write) => write.archiveTask(task.id),
    onDone: () => ctx.refresh(),
  });
}

/** T-12's 已归档 drawer. Restore moves the directory back under docs/. */
function openArchiveDrawer(archived, ctx) {
  const body = [
    h('div.quiet.mb-12', {
      text: '归档 = 任务目录移入 docs/.archive/，从列表移出；合并记录仍在默认分支的历史里。恢复会把目录移回 docs/。',
    }),
  ];
  if (!archived.length) {
    body.push(empty('还没有归档过的任务。'));
  } else {
    const stack = h('div.stack.stack--tight');
    for (const task of archived) {
      const row = h('div.archrow', [h('b', { text: task.id })]);
      const middle = h('span', { text: task.title || task.slug });
      middle.appendChild(
        h('small', {
          text: task.merge_commit
            ? `合并 ${task.merge_commit} · ${labels.day(task.completed_at)} · docs/.archive/${task.slug}/`
            : `docs/.archive/${task.slug}/`,
        })
      );
      row.appendChild(middle);
      row.appendChild(
        registerWrite(
          h('button.btn.btn--sm.btn--text', {
            type: 'button',
            onClick: () =>
              attempt({
                label: `已恢复 ${task.id}`,
                success: `目录已移回 docs/${task.slug}/，回到「已完成」。`,
                run: (write) => write.restoreTask(task.id),
                onDone: () => {
                  close();
                  return ctx.refresh();
                },
              }),
          }, [text('恢复')])
        )
      );
      stack.appendChild(row);
    }
    body.push(stack);
  }
  body.push(
    h('div.quiet.mt-12', { text: '移动目录只落在默认分支工作树，提交由你自己做，与改配置同一规则。' })
  );
  openDrawer({ title: '已归档', body });
}

// ---------------------------------------------------------------------------
// 新建任务 drawer — requirement U-05 / T-01, T-02
// ---------------------------------------------------------------------------

function newTaskTile(data, ctx) {
  const tile = h('div.card.card--dashed.taskcard.ptile--new.card--link', {
    tabindex: '0',
    role: 'button',
    onClick: () => openNewTaskDrawer(data, ctx),
    onKeyDown: activateOnKey,
  });
  tile.appendChild(h('span.big', [icon('plus', 'ic--lg')]));
  const label = h('div', [h('b', { text: '新建任务' }), h('br')]);
  label.appendChild(h('span.small', { text: '一句话需求 + 附件' }));
  tile.appendChild(label);
  return registerWrite(tile);
}

export function openNewTaskDrawer(data, ctx) {
  const project = (data && data.project) || {};
  const tasks = (data && data.tasks) || [];
  const counts = (data && data.counts) || {};
  const slotsInUse = tasks.filter((t) => t.state && t.state.state === 'active').length;
  const limit = project.parallel_limit || 0;
  const full = slotsInUse >= limit;

  const request = h('textarea', {
    class: 'textarea--tall',
    placeholder: '例如：退款流程支持部分退款，退款成功后库存回补，不改支付网关。',
  });
  // T-01's attachments are copied into the worktree by the core, which needs a
  // path. The preload exposes no way to turn a dropped File into one (Electron
  // no longer surfaces `File.path` to a sandboxed renderer and there is no
  // path-resolving bridge), so the field asks for paths outright rather than
  // pretending a drop worked.
  const attachments = h('textarea', {
    class: 'textarea--short',
    placeholder: '/Users/you/Downloads/refund-policy.pdf',
  });
  const docRefs = h('textarea', {
    class: 'textarea--short',
    placeholder: 'docs/payments/refund-notes.md',
  });

  const header = h('div.row.mb-12', [
    tag(project.display_name || project.id, 'soft-blue'),
    tag(`从 ${project.default_branch || 'main'} 切分支`, 'outlined'),
    full
      ? tag(`并行 ${slotsInUse} / ${limit} 已满 · 将排队 #${(counts.queued || 0) + 1}`, 'soft-yellow')
      : tag(`并行 ${slotsInUse} / ${limit} · 有槽位，立即启动`, 'soft-green'),
  ]);

  const submit = registerWrite(
    h('button.btn.btn--yellow.ml-auto', {
      type: 'button',
      onClick: () => {
        const text0 = request.value.trim();
        if (!text0) {
          request.focus();
          return undefined;
        }
        return attempt({
          label: '已创建任务',
          success: full
            ? '并行已满，已排队 · 任一任务释放槽位后自动启动。'
            : '任务整理节点已开始调研仓库并起草任务文件。',
          run: (write) =>
            write.createTask({
              projectId: project.id,
              request: text0,
              attachments: lines(attachments.value),
              docRefs: lines(docRefs.value),
            }),
          onDone: () => {
            close();
            return ctx.refresh();
          },
        });
      },
    }, [text('创建并启动')])
  );

  openDrawer({
    title: '新建任务',
    body: [
      header,
      h('div.sec.sec--flush', [h('h3', { text: '一句话需求' })]),
      h('label.input.input--area', [request]),
      h('div.sec.sec--gap', [
        h('h3', { text: '附件' }),
        h('span.muted', { text: '每行一个绝对路径 · 复制进任务目录，随分支提交' }),
      ]),
      h('label.input.input--area', [attachments]),
      h('div.sec.sec--gap', [
        h('h3', { text: '仓库内文档' }),
        h('span.muted', { text: '每行一个仓库内路径 · 引用，不复制' }),
      ]),
      h('label.input.input--area', [docRefs]),
      h('div.quiet.mt-12', {
        text: '提交后由任务整理节点调研仓库、生成任务文件并起标题，不再确认。第一次停顿在设计定稿后。',
      }),
    ],
    footer: [closeButton('取消'), submit],
  });
  request.focus();
}

function lines(value) {
  return String(value || '')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
}
