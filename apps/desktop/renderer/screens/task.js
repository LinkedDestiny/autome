// 任务面板 — requirement U-06 / section 4.5.
//
// The panel is the whole of one task on one screen: the header, the 13-node
// flow (T-03), the current session, the milestones (T-11's primary progress),
// the untouched original request, 待你决定 (T-09), the stopping panel, the
// produced documents (T-13) and the session history (T-14).
//
// The stopping panel has exactly four faces, and which one shows is decided by
// the task's state alone — never by "we just clicked approve, so probably".
// A task that is running says so and offers nothing; that is the honest answer
// to "what do I do now", and the design document says it in words.
//
// `task.get` is one call by design: the panel needs the state, the document's
// status block and the session list to agree, and three separate reads could
// disagree mid-render.

import {
  h, icon, text, reveal, tag, spinnerTag, progress, repoList, cardHead,
  empty, activateOnKey, clear,
} from '../lib/dom.js';
import { read, readOr, attempt, registerWrite } from '../lib/api.js';
import { openDrawer, openModal, closeButton, close } from '../lib/overlay.js';
import * as labels from '../lib/labels.js';

export const id = 'task';
export const nav = 'projects';

export async function load(ctx) {
  const taskId = ctx.params.taskId;
  const payload = await read('getTask', taskId);
  const node = labels.stateNode(payload.task && payload.task.state);
  // The file list and the merge preconditions are only meaningful at the
  // merge stop, and computing them runs git — so it is asked for only there.
  const changes = node === 'await_merge' ? await readOr(null, 'taskChanges', taskId) : null;
  return { ...payload, changes };
}

export function render(host, data, ctx) {
  const screen = h('div.screen.active', { 'data-screen': 'task' });
  const task = (data && data.task) || {};
  const project = (data && data.project) || {};
  const status = data && data.status_block;
  const node = labels.stateNode(task.state);

  screen.appendChild(
    h('div.crumb', [
      h('button', { type: 'button', onClick: () => ctx.navigate('projects') }, [text('项目')]),
      h('span.sep', { text: '›' }),
      h('button', {
        type: 'button',
        onClick: () => ctx.navigate('project', { projectId: project.id }),
      }, [text(project.name || project.id || '项目')]),
      h('span.sep', { text: '›' }),
      h('b', { text: task.id || '' }),
    ])
  );

  screen.appendChild(reveal(hero(data, ctx), 0));

  screen.appendChild(
    h('div.cardgrid.cardgrid--4.mt-12', [
      reveal(currentSessionCard(data), 1),
      reveal(milestonesCard(status, data), 2),
      reveal(requestCard(task), 3),
      reveal(decisionsCard(data, ctx), 4),
    ])
  );

  screen.appendChild(
    h('div.cardgrid.cardgrid--3.mt-12', [
      reveal(stopCard(data, ctx, node), 5),
      reveal(usageCard(data), 6),
      reveal(documentsCard(data), 7),
    ])
  );

  screen.appendChild(h('div.cardgrid.cardgrid--3.mt-12', [reveal(sessionsCard(data), 8)]));

  host.appendChild(screen);
  // After the append: the title has no width until it is in the document.
  markTitleOverflow(screen);
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

function hero(data, ctx) {
  const task = (data && data.task) || {};
  const status = data && data.status_block;
  const state = task.state || {};
  const node = labels.stateNode(state);
  const pill = labels.taskStatus(task);

  const card = h('div.card.hero.card--pattern.card--pattern-teal');

  // Band 1 — what it is, and the buttons that act on it. One line: the title
  // is the raw request and can be a paragraph long, and letting it wrap pushes
  // everything below it around as tasks come and go. The full text is in the
  // tooltip and, in full, in the task document.
  card.appendChild(
    h('div.hero__top', [
      heroTitle(task),
      heroActions(data, ctx),
    ])
  );

  // Band 2 — how far along, and where it lives. The state pill and the round
  // counters answer the same question and belong on the same line; they used
  // to sit in two rows with the whole flow diagram between them.
  //
  // Branch/worktree/age ride along at the end of this row rather than getting
  // a row of their own: a fourth band pushed the screen past the 944px U-11
  // holds it to, and they are the least urgent thing on the card — a quiet
  // line beside the chips says that better than three outlined tags did, the
  // widest of which was the path.
  const meta = h('div.hero__meta');
  meta.appendChild(pill.spinning ? spinnerTag(pill.label, pill.variant) : tag(pill.label, pill.variant, pill.icon));
  if (state.state === 'queued' && data.queue_position) {
    meta.appendChild(tag(`排队 #${data.queue_position}`, 'soft-yellow'));
  }
  roundTags(status, task, data).forEach((t) => meta.appendChild(t));
  meta.appendChild(heroIdent(task, data));
  card.appendChild(meta);

  // Band 3 — where it is in the flow, across the full width of the card.
  card.appendChild(flowRoute(node, state));
  return card;
}

/**
 * The title, clamped to one line and expandable by clicking it.
 *
 * The title is the task's whole request sentence. Left to wrap it is three
 * lines of heading and every band below it moves as tasks come and go; cut to
 * one line it is unreadable exactly when it matters — a long request whose
 * distinguishing half is past the ellipsis. So it is one line by default and
 * the full text on demand.
 *
 * A real `<button>` rather than a click handler on the `<h2>`: this is the one
 * thing on the card you can operate with the keyboard and not see, and the
 * button gets focus, Enter/Space and a name from the browser for free.
 *
 * `markTitleOverflow` decides afterwards whether the control is offered at
 * all — a short title has nothing to expand, and a chevron that does nothing
 * when clicked is worse than no chevron.
 */
function heroTitle(task) {
  const full = `${task.id || ''} · ${task.title || task.request || task.slug || ''}`;
  const label = h('span.hero__title-text', { text: full });
  const btn = h(
    'button.hero__title-btn',
    {
      type: 'button',
      'aria-expanded': 'false',
      onClick: (e) => {
        const el = e.currentTarget;
        const open = el.getAttribute('aria-expanded') === 'true';
        el.setAttribute('aria-expanded', open ? 'false' : 'true');
      },
    },
    [label, icon('chev', 'hero__title-chev')]
  );
  return h('h2.hero__title', { title: full }, [btn]);
}

/**
 * Marks the titles that actually overflow, once they are laid out.
 *
 * Width is not knowable while the card is being built, so this runs after the
 * screen is in the document. Until it does the control is hidden, which is the
 * safe way round: a title that turns out to need expanding gains the chevron a
 * frame later, rather than every title showing one and most doing nothing.
 */
function markTitleOverflow(host) {
  requestAnimationFrame(() => {
    host.querySelectorAll('.hero__title-text').forEach((el) => {
      const over = el.scrollWidth > el.clientWidth + 1;
      el.closest('.hero__title').classList.toggle('hero__title--over', over);
    });
  });
}

/** `/Users/dannie/project/x` → `~/project/x`. The home prefix is the same on
 *  every row and pushes the part that differs off the end of the line. */
function tildeHome(path) {
  return String(path).replace(/^\/Users\/[^/]+\//, '~/');
}

/** Branch, worktree and age: the identifying facts, none of them urgent. */
function heroIdent(task, data) {
  const row = h('div.hero__ident');
  const parts = [];
  if (task.branch) parts.push(h('span', { text: `分支 ${task.branch}` }));
  if (data && data.worktree) {
    parts.push(h('span', { text: tildeHome(data.worktree), title: data.worktree }));
  }
  if (task.created_at) {
    parts.push(
      h('span', {
        text: `创建 ${labels.clock(task.created_at)} · 已存在 ${labels.duration(task.created_at)}`,
      })
    );
  }
  parts.forEach((part, i) => {
    if (i) row.appendChild(h('span.hero__ident-sep', { text: '·' }));
    row.appendChild(part);
  });
  return row;
}

/**
 * T-08's three interventions plus the terminal jump. Which are offered is a
 * function of the state: pausing a queued task or stopping a finished one
 * would be a button that can only fail.
 */
function heroActions(data, ctx) {
  const task = (data && data.task) || {};
  const state = task.state || {};
  const row = h('div.hero__actions');
  const running = state.state === 'active';

  // All three carry the same shape and differ only in colour: teal acts on the
  // loop, blue opens a window onto it, red ends it. Weight used to carry the
  // meaning instead — two filled buttons and a bare one — which said these
  // were three unrelated controls rather than three things you can do here.
  if (running) {
    row.appendChild(
      registerWrite(
        h('button.btn.btn--sm.btn--go', {
          type: 'button',
          onClick: () =>
            attempt({
              label: '已请求暂停',
              success: '当前会话跑完后不再启动下一节点。',
              run: (write) => write.pauseTask(task.id),
              onDone: () => ctx.refresh(),
            }),
        }, [icon('pause'), text('暂停')])
      )
    );
    row.appendChild(
      registerWrite(
        h('button.btn.btn--sm.btn--go', {
          type: 'button',
          onClick: () =>
            attempt({
              label: '已停止',
              success: '当前会话已被终止；任务停在这个节点，可随时继续。',
              run: (write) => write.stopTask(task.id),
              onDone: () => ctx.refresh(),
            }),
        }, [icon('x'), text('停止')])
      )
    );
  }
  if (state.state === 'paused' || state.state === 'stopped') {
    row.appendChild(
      registerWrite(
        h('button.btn.btn--sm.btn--go', {
          type: 'button',
          onClick: () =>
            attempt({
              label: '已继续',
              success: '下一个节点会用当前配置启动。',
              run: (write) => write.resumeTask(task.id),
              onDone: () => ctx.refresh(),
            }),
        }, [icon('play'), text('继续')])
      )
    );
  }
  if (!['done', 'cancelled'].includes(state.state)) {
    // Opening a terminal is a utility, not the thing you came here to do, so
    // it does not carry the same weight as 继续/暂停. Two filled buttons of
    // equal weight with a bare one between them read as three unrelated
    // controls.
    row.appendChild(
      registerWrite(
        h('button.btn.btn--sm.btn--util', {
          type: 'button',
          onClick: () =>
            attempt({
              label: '已打开终端',
              success: '终端停在这个任务的 worktree，并显示最近一次会话的日志；正在跑的会话会实时跟。',
              run: (write) => write.openTerminal(task.id),
            }),
        }, [icon('term'), text('打开终端')])
      )
    );
    // Last, and set apart: the one button here you cannot undo.
    row.appendChild(
      registerWrite(
        h('button.btn.btn--sm.btn--danger.hero__actions-last', {
          type: 'button',
          onClick: () => confirmCancel(task, ctx),
        }, [text('取消')])
      )
    );
  }
  return row;
}

function confirmCancel(task, ctx) {
  openModal({
    title: '取消任务',
    lead: `${task.id} · ${task.title || task.request || ''}`,
    body: [
      h('div.quiet', {
        text: `取消会终止当前会话，删除 worktree 与分支 ${task.branch || ''}，任务文档复制到 docs/.archive/${task.slug || ''}/。默认分支不受影响。`,
      }),
    ],
    footer: [
      closeButton('再想想'),
      registerWrite(
        h('button.btn.btn--danger.ml-auto', {
          type: 'button',
          onClick: () =>
            attempt({
              label: `已取消 ${task.id}`,
              success: 'worktree 与分支已删除，文档已归档。',
              run: (write) => write.cancelTask(task.id),
              onDone: () => {
                close();
                return ctx.refresh();
              },
            }),
        }, [text('确认取消')])
      ),
    ],
  });
}

/** T-03's fixed 13-node flow. Position comes from the state, nothing else. */
function flowRoute(node, state) {
  const route = h('div.route.route--hero.route--13');
  const terminal = state.state === 'done';
  const currentIndex = terminal
    ? labels.FLOW.length - 1
    : labels.FLOW.findIndex((s) => s.key === node);

  labels.FLOW.forEach((step, index) => {
    const done = currentIndex >= 0 && index < currentIndex;
    const now = index === currentIndex;
    const stone = h(`div.stone${done ? '.done' : ''}${now ? '.now' : ''}`);
    const dot = h('div.stone__dot');
    if (done || (now && terminal)) dot.appendChild(icon('check'));
    stone.appendChild(dot);
    stone.appendChild(h('div.stone__lbl', { text: step.label }));
    route.appendChild(stone);
  });
  return route;
}

/** T-11's secondary indicators. Absent when the status block could not be read
 *  — an unreadable document means we do not know the round, and saying "1/15"
 *  would be an invention. */
function roundTags(status, task, data) {
  if (!status) {
    return [
      tag(
        data && data.status_error
          ? '设计文档的状态块还读不到 · 轮次未知'
          : '设计文档还没有生成 · 轮次未知',
        'dashed-brown'
      ),
    ];
  }
  const budget = task.budget_n || status.impl_round_limit;
  const tags = [
    tag(`设计循环 ${status.design_round} / ${status.design_round_limit} 轮`, 'soft-green'),
    tag(`实现循环 ${status.impl_round} / ${budget} 轮`, 'soft-teal'),
    tag(
      `reopen ${status.current_milestone_reopens || 0} · convergence ${status.convergence_mode}`,
      'soft-brown'
    ),
  ];
  const backlog = ((data && data.decisions) || []).filter((d) => d.kind === 'backlog').length;
  const disputes = ((data && data.decisions) || []).filter((d) => d.kind === 'dispute').length;
  if (backlog || disputes) {
    tags.push(tag(`Backlog ${backlog} · 争议项 ${disputes}`, 'soft-brown'));
  }
  return tags;
}

// ---------------------------------------------------------------------------
// The four upper cards
// ---------------------------------------------------------------------------

function currentSessionCard(data) {
  const sessions = (data && data.sessions) || [];
  const running = sessions.find((s) => s.running);
  const status = data && data.status_block;

  const card = h('div.card.card--pad.col');
  card.appendChild(
    cardHead(
      '当前会话',
      running
        ? spinnerTag(labels.duration(running.started_at) || '刚开始', 'solid-teal')
        : tag('没有会话在跑', 'outlined')
    )
  );

  if (!running) {
    card.appendChild(
      h('div.quiet', {
        text: '当前没有 CLI 会话。任务要么停在人工节点，要么正在跑一个系统步骤（rebase、合并、清理）。',
      })
    );
    if (status && status.next_action) {
      card.appendChild(h('div.quiet.mt-8', { text: `next-action：${status.next_action}` }));
    }
    return card;
  }

  const rows = [
    ['角色', `${running.label || ''} · 第 ${running.round} 轮`],
    ['CLI', `${labels.runtimeLabel(running.runtime)} · ${running.model}${running.effort ? ` · ${running.effort}` : ''}`],
    ['技能', (running.skills || []).length ? running.skills.join(' · ') : '未绑定 · 自由选择'],
  ];
  if (status && status.current_milestone) {
    rows.push(['当前里程碑', `${status.current_milestone} · reopen ${status.current_milestone_reopens || 0}`]);
  }
  if (status && status.next_action) rows.push(['next-action', status.next_action]);
  card.appendChild(repoList(rows, 'repo--wrap'));
  return card;
}

/** T-11's primary progress: milestones done over total, from the design doc. */
function milestonesCard(status, data) {
  const card = h('div.card.card--pad.col');
  if (!status) {
    card.appendChild(cardHead('里程碑', tag('未知', 'dashed-brown')));
    /* The core names the offending field or the offending row and its line
     * number. Printing only "读不出来" left the user with a 240KB document and
     * no idea which line of it to look at. */
    const detail = data && data.status_error_detail;
    card.appendChild(
      empty(
        detail
          ? `设计文档的状态块读不出来：${detail}`
          : data && data.status_error
            ? '设计文档的状态块读不出来，里程碑无从得知。'
            : '设计还没有产出里程碑。'
      )
    );
    return card;
  }
  const done = status.milestones_done || 0;
  const total = status.milestones_total || 0;
  card.appendChild(cardHead('里程碑', h('span.muted.small', { text: `${done} / ${total} 已完成` })));

  const list = h('div.mstones');
  for (const milestone of status.milestones || []) {
    const row = h('div.mstone', [h('b', { text: milestone.id })]);
    const title = milestone.reopen_count
      ? `${milestone.title} · reopen ${milestone.reopen_count}`
      : milestone.title;
    row.appendChild(h('span', { text: title }));
    const variant =
      milestone.state === 'done' ? 'soft-green' : milestone.state === 'pending' ? 'soft-yellow' : 'outlined';
    row.appendChild(tag(labels.MILESTONE_LABELS[milestone.state] || milestone.state, variant));
    list.appendChild(row);
  }
  card.appendChild(list.childNodes.length ? list : empty('设计文档里还没有里程碑。'));
  if (total) {
    const bar = progress(done / total, `${Math.round((done / total) * 100)}%`);
    bar.classList.add('mt-8');
    card.appendChild(bar);
  }
  return card;
}

/// What the task has cost so far, and which version of the rules it is being
/// held to.
///
/// Every number here is absent rather than zero when nothing measured it. A
/// task run entirely on Codex has an unknown cost — Codex reports no price —
/// and a dash says that where `$0.00` would be a claim.
function usageCard(data) {
  const task = (data && data.task) || {};
  const soFar = (data && data.so_far) || {};
  const metrics = task.metrics;
  const card = h('div.card.card--pad.col', [
    cardHead('用量', tag(metrics ? '已终结' : '进行中', metrics ? 'outlined' : 'soft-blue')),
  ]);

  const rows = [
    ['协议版本', labels.protocolRef(task.protocol_ref)],
    ['费用', labels.cost(metrics ? metrics.total_cost_usd : soFar.total_cost_usd)],
    ['tokens', labels.tokens(metrics ? metrics.total_tokens : soFar.total_tokens)],
    ['turns', labels.count(metrics ? metrics.total_turns : soFar.total_turns)],
  ];
  if (metrics) {
    rows.push(['设计轮', labels.ratio(metrics.design_rounds_used, metrics.design_rounds_limit)]);
    rows.push(['实现轮', labels.ratio(metrics.impl_rounds_used, metrics.budget_n)]);
    rows.push(['reopen', labels.count(metrics.reopen_total)]);
    rows.push(['实现缺陷 / 验证缺口',
      `${labels.count(metrics.impl_defects)} / ${labels.count(metrics.verification_gaps)}`]);
    if (metrics.closed_then_contradicted) {
      rows.push(['关闭后被推翻', labels.count(metrics.closed_then_contradicted)]);
    }
  } else {
    const status = data.status_block;
    if (status) {
      rows.push(['设计轮', labels.ratio(status.design_round, status.design_round_limit)]);
      rows.push(['实现轮', labels.ratio(status.impl_round, status.impl_round_limit)]);
    }
  }
  card.appendChild(repoList(rows));

  if (!metrics && !soFar.sessions_measured) {
    card.appendChild(
      h('div.quiet.mt-8', {
        text: '还没有会话留下用量。CLI 的原始事件流读不出来时这里是空的——空着比填 0 诚实。',
      })
    );
  }
  if (metrics && typeof metrics.total_cost_usd !== 'number') {
    card.appendChild(
      h('div.quiet.mt-8', { text: '费用未知：Codex 不报价，Autome 不自己编一张价格表。' })
    );
  }
  return card;
}

/** T-01's one-line request, verbatim and read-only for the task's whole life. */
function requestCard(task) {
  const card = h('div.card.card--pad.col', [cardHead('原始需求', tag('只读', 'outlined'))]);
  card.appendChild(h('div.rawreq.rawreq--sm', { text: task.request || '（没有记录原始需求）' }));
  return card;
}

function decisionsCard(data, ctx) {
  const decisions = (data && data.decisions) || [];
  const open = decisions.filter((d) => !d.consumed);
  const undecided = open.filter((d) => d.disposition === 'none');

  const card = h('div.card.card--pattern.card--pattern-purple.card--pad.col.card--link', {
    tabindex: '0',
    role: 'button',
    onClick: () => openDecideDrawer(data, ctx),
    onKeyDown: activateOnKey,
  });
  card.appendChild(cardHead('待你决定', tag(String(open.length), 'solid-purple')));

  if (!open.length) {
    card.appendChild(h('div.quiet.quiet--inherit', { text: '没有争议项，也没有等着处置的 Backlog。' }));
    return card;
  }

  const body = h('div.quiet.quiet--inherit');
  for (const item of open.slice(0, 3)) {
    body.appendChild(h('b', { text: item.kind === 'dispute' ? '争议项 ' : 'Backlog ' }));
    body.appendChild(text(`${item.item_id} ${item.text}`));
    body.appendChild(h('br'));
  }
  card.appendChild(body);
  const cta = h('span.card__cta', {
    text: undecided.length ? `${undecided.length} 条还没表态 · 在停顿点消费 ` : '已表态 · 在停顿点消费 ',
  });
  cta.appendChild(icon('arrow', 'ic--sm'));
  card.appendChild(cta);
  return card;
}

/**
 * T-09. A stance never interrupts a running task; it is consumed at the next
 * stopping point, and the drawer says so rather than implying an immediate
 * effect.
 */
function openDecideDrawer(data, ctx) {
  openDrawer({ title: '待你决定', body: decideBody(data, ctx), footer: [closeButton('关闭')] });
}

function decideBody(data, ctx) {
  const task = (data && data.task) || {};
  const decisions = ((data && data.decisions) || []).filter((d) => !d.consumed);

  const body = [
    h('div.quiet.mb-12', {
      text: `${task.id} · ${decisions.length} 条 · 表态不会打断运行，在下一个停顿点消费。纳入 Backlog 会让任务退回实现。`,
    }),
  ];

  if (!decisions.length) {
    body.push(empty('评审与审计还没有留下需要你处置的条目。'));
  } else {
    const stack = h('div.stack.stack--loose');
    for (const item of decisions) stack.appendChild(decideRow(task, item, ctx));
    body.push(stack);
  }
  return body;
}

/**
 * Re-reads the task and rebuilds the drawer's *body* after a stance is saved.
 *
 * Taking a stance used to close the drawer, which made triaging a list of
 * Backlog items one at a time: decide, drawer shuts, find the button, open it
 * again. Nothing about a stance requires the drawer to close — it is consumed
 * at the next stopping point, not now.
 *
 * Only the body is replaced, not the drawer: `openDrawer` would rebuild the
 * panel and replay its slide-in. And it is replaced from a fresh read rather
 * than by marking the clicked pill locally, so the drawer still shows the
 * core's answer and not our assumption about it.
 */
async function refreshDecideDrawer(taskId, ctx) {
  if (!document.querySelector('#drawer .drawer__body')) return;
  const fresh = await readOr(null, 'getTask', taskId);
  // The read is awaited, so the user may have closed the drawer meanwhile.
  const bodyEl = document.querySelector('#drawer .drawer__body');
  if (!fresh || !bodyEl) return;
  clear(bodyEl);
  for (const node of decideBody(fresh, ctx)) bodyEl.appendChild(node);
}

function decideRow(task, item, ctx) {
  const row = h('div.decide', [
    h('div.decide__k', {
      text: item.kind === 'dispute' ? `争议项 · ${item.item_id}` : `Backlog · ${item.item_id}`,
    }),
    h('div.decide__t', { text: item.text }),
  ]);
  if (item.ruling) row.appendChild(h('div.decide__d', { text: `已写裁定：${item.ruling}` }));

  const pick = h('div.pick');
  const options =
    item.kind === 'dispute'
      ? [['ruled', '写裁定…'], ['none', '暂不裁定']]
      : [['include', '纳入 · 成为新里程碑'], ['ignore', '忽略 · 写入 retro'], ['none', '暂不处置']];

  for (const [disposition, label] of options) {
    const pill = tag(label, 'outlined');
    if (item.disposition === disposition) pill.classList.add('on');
    pill.setAttribute('role', 'button');
    pill.setAttribute('tabindex', '0');
    pill.addEventListener('keydown', activateOnKey);
    pill.addEventListener('click', () => {
      if (disposition === 'ruled') {
        promptRuling(task, item, ctx);
        return;
      }
      attempt({
        label: '表态已保存',
        success: '到下一个停顿点时消费。',
        run: (write) =>
          write.decide({ taskId: task.id, kind: item.kind, itemId: item.item_id, disposition }),
        onDone: async () => {
          // Screen first, then the drawer: the screen render calls
          // `resetWriteControls`, so the drawer's new pills must be built
          // after it or they would not be registered as write controls.
          await ctx.refresh();
          await refreshDecideDrawer(task.id, ctx);
        },
      });
    });
    registerWrite(pill);
    pick.appendChild(pill);
  }
  row.appendChild(pick);
  return row;
}

function promptRuling(task, item, ctx) {
  const field = h('textarea', { class: 'textarea--short', placeholder: '写下你的裁定…' });
  if (item.ruling) field.value = item.ruling;
  openModal({
    title: `裁定 ${item.item_id}`,
    lead: item.text,
    body: [h('label.input.input--area', [field])],
    footer: [
      closeButton('取消'),
      registerWrite(
        h('button.btn.btn--yellow.ml-auto', {
          type: 'button',
          onClick: () => {
            const ruling = field.value.trim();
            if (!ruling) {
              field.focus();
              return undefined;
            }
            return attempt({
              label: '裁定已保存',
              success: '下一轮设计会带着它重跑。',
              run: (write) =>
                write.decide({
                  taskId: task.id,
                  kind: item.kind,
                  itemId: item.item_id,
                  disposition: 'ruled',
                  ruling,
                }),
              onDone: async () => {
                // The modal replaced the drawer, so this reopens it rather
                // than refreshing it — a ruling is usually one of several
                // items to work through.
                close();
                await ctx.refresh();
                const fresh = await readOr(null, 'getTask', task.id);
                if (fresh) openDecideDrawer(fresh, ctx);
              },
            });
          },
        }, [text('保存裁定')])
      ),
    ],
  });
  field.focus();
}

// ---------------------------------------------------------------------------
// The stopping panel — four faces (U-06)
// ---------------------------------------------------------------------------

function stopCard(data, ctx, node) {
  const task = (data && data.task) || {};
  const state = task.state || {};
  const card = h('div.card.card--pad.col');

  if (state.state === 'failed') {
    card.appendChild(cardHead('停在失败上，怎么办', tag('等待你', 'solid-red')));
    failedFace(card, data, ctx);
  } else if (node === 'await_design_approval') {
    card.appendChild(cardHead('批准设计', tag('等待你', 'solid-yellow')));
    humanApprovalNotice(card, data);
    approveFace(card, data, ctx);
  } else if (node === 'await_merge') {
    card.appendChild(cardHead(`合并到 ${(data.project || {}).default_branch || 'main'}`, tag('等待你', 'solid-green')));
    humanApprovalNotice(card, data);
    mergeFace(card, data, ctx);
  } else {
    card.appendChild(cardHead('停顿面板', tag('当前无需操作', 'outlined')));
    idleFace(card, state);
  }
  return card;
}

/// Changes the review and audit rounds were not competent to judge.
///
/// Only ever non-empty for a meta task — one that edits the protocol itself —
/// and then only for edits to those two rounds' own prompts. There is no
/// clever fix for the self-reference: an evaluator judging the rules it is
/// evaluated under is a fixed point, not a check. So it lands here, at both
/// gates, where a person is already looking.
function humanApprovalNotice(card, data) {
  const files = (data && data.needs_human_approval) || [];
  if (!files.length) return;
  card.appendChild(
    h('div.alertbar', [text(`需人工特批：${files.join('、')}`)])
  );
  card.appendChild(
    h('div.quiet', {
      text: '这次改动动了评审轮或审计轮自己的 prompt。它们判不了这个——一个评测者去审自己被评测所依据的规则，那是不动点不是检查。这一条只能你看。',
    })
  );
}

function idleFace(card, state) {
  if (state.state === 'done') {
    card.appendChild(h('div.quiet', { text: '任务已经合并进默认分支，没有什么要做的了。可以在项目页把它归档。' }));
    return;
  }
  if (state.state === 'cancelled') {
    card.appendChild(h('div.quiet', { text: '任务已取消，worktree 与分支已删除，文档保留在归档目录里。' }));
    return;
  }
  card.appendChild(
    h('div.quiet', {
      text: '现在不需要你做什么。下一次停顿在设计定稿，或者 rebase 之后的「待合并」，届时这里会出现按钮。',
    })
  );
  card.appendChild(
    h('div.quiet.mt-8', {
      text: '如果要改方向：停止 → 修改需求或规则 → 重开任务。运行中不接受口头纠偏。',
    })
  );
}

/** T-04. Approve moves on; reject goes back to design carrying the feedback. */
function approveFace(card, data, ctx) {
  const task = data.task || {};
  const status = data.status_block;
  const pending = data.pending_decisions || {};
  const hasPending = (pending.included || 0) + (pending.ruled || 0) > 0;

  card.appendChild(
    h('div.quiet', {
      text: status
        ? `设计文档已定稿 · 设计 ${status.design_round} / ${status.design_round_limit} 轮 · ${status.milestones_total} 个里程碑${hasPending ? ' · 有未消费的表态：批准会变成「带着决定再跑一轮设计」' : ''}`
        : '设计已定稿，但状态块读不出来。批准前建议先看一眼设计文档。',
    })
  );

  const designDoc = (data.documents || []).find((d) => d.name === `${task.slug}.md`);
  const row = h('div.row.gap-6.mt-8');
  if (designDoc) {
    row.appendChild(
      registerWrite(
        h('button.btn.btn--sm', {
          type: 'button',
          onClick: () => openDocument(task, designDoc),
        }, [icon('doc'), text('看设计文档')])
      )
    );
  }
  row.appendChild(
    registerWrite(
      h('button.btn.btn--sm.btn--primary', {
        type: 'button',
        onClick: () =>
          attempt({
            label: '已批准设计',
            success: hasPending
              ? '带着未消费的决定再跑一轮设计，定稿后再停一次给你看。'
              : '进入实现循环。',
            run: (write) => write.approveTask(task.id),
            onDone: () => ctx.refresh(),
          }),
      }, [text('批准，进入实现')])
    )
  );
  card.appendChild(row);

  const feedback = h('textarea', {
    class: 'textarea--short',
    placeholder: '驳回意见：例如「确认邮件只发给登录用户，游客下单不发」…',
  });
  card.appendChild(h('label.input.input--area.mt-8', [feedback]));
  card.appendChild(
    h('div.row.mt-6', [
      registerWrite(
        h('button.btn.btn--sm.btn--text', {
          type: 'button',
          onClick: () => {
            const value = feedback.value.trim();
            if (!value) {
              feedback.focus();
              return undefined;
            }
            return attempt({
              label: '已驳回',
              success: '意见已写入，回到设计节点重跑评审与裁决，定稿后再停。',
              run: (write) => write.rejectTask(task.id, value),
              onDone: () => ctx.refresh(),
            });
          },
        }, [text('驳回并附意见')])
      ),
    ])
  );
}

/** T-05/T-07. The merge button exists only when the core says it may. */
function mergeFace(card, data, ctx) {
  const task = data.task || {};
  const changes = data.changes;

  if (!changes || !changes.available) {
    card.appendChild(
      empty('分支已经不在了，或者变更统计读不出来。刷新一次；如果还是这样，任务需要从实现节点重跑。')
    );
    return;
  }

  // A Backlog item marked 纳入 turns into new work *at this stopping point*
  // (T-09): the core sends the task back to the implementation loop and grows
  // its budget instead of merging. A button that says 合并到 main and then
  // does that is lying, and it lied to a real user twice — once on 09-17 and
  // again today, each time costing a round that ended in a protocol failure.
  const included = (changes.pending_decisions || {}).included || 0;

  const rows = [
    ['分支', `${changes.branch} → ${changes.into}`],
    ['提交', `${changes.commits} commits · ${changes.files.length} 文件 +${changes.total_added} −${changes.total_deleted}`],
    ['主工作树', changes.mergeable ? '干净 · 可以合并' : blockedText(changes.blocked_by)],
  ];
  if (included) rows.push(['Backlog', backlogText(included)]);
  card.appendChild(repoList(rows));

  const row = h('div.row.gap-6.mt-8');
  row.appendChild(
    h('button.btn.btn--sm', {
      type: 'button',
      onClick: () => openFilesDrawer(changes),
    }, [text('看文件列表')])
  );

  const mergeButton = registerWrite(
    h('button.btn.btn--yellow.btn--sm', {
      type: 'button',
      onClick: () =>
        attempt({
          label: included ? '已回到实现轮' : `已合并到 ${changes.into}`,
          success: included
            ? `${included} 个 Backlog 项会先做完，做完后回到这里。`
            : 'worktree 与分支已清理，任务完成。',
          run: (write) => write.mergeTask(task.id),
          onDone: () => ctx.refresh(),
        }),
    }, [text(included ? `先做 ${included} 个 Backlog 项` : `合并到 ${changes.into}`)])
  );
  // T-07's preconditions are the core's to enforce, but offering a button that
  // can only fail is worse than saying why it is not offered.
  if (!changes.mergeable) {
    mergeButton.disabled = true;
    mergeButton.title = blockedText(changes.blocked_by);
  }
  row.appendChild(mergeButton);
  card.appendChild(row);
}

/**
 * What N included Backlog items mean for the button next to this line.
 *
 * Said in the panel rather than only in the button, because the button is
 * read last: by the time someone reaches it they have already decided from
 * the rows above that this is a merge.
 */
function backlogText(included) {
  return `${included} 项标记为纳入 · 会先变成新里程碑做完，再回到这里`;
}

function blockedText(blocked) {
  if (!blocked) return '不满足合并前置条件';
  if (blocked.kind === 'dirty_worktree') {
    return `主工作树有未提交改动：${(blocked.paths || []).slice(0, 3).join(' · ')}`;
  }
  if (blocked.kind === 'needs_rebase') return '任务分支还没有 rebase 到最新默认分支';
  return blocked.kind;
}

/** T-10's three options: extend, rerun from a node, or cancel. */
function failedFace(card, data, ctx) {
  const task = data.task || {};
  const state = task.state || {};
  card.appendChild(h('div.quiet', { text: labels.failureReason(state.reason) }));

  const row = h('div.row.gap-6.mt-8');
  row.appendChild(
    registerWrite(
      h('button.btn.btn--sm.btn--primary', {
        type: 'button',
        onClick: () =>
          attempt({
            label: '已追加 5 轮',
            success: '预算上限加 5，从停下的节点继续。',
            run: (write) => write.extendBudget(task.id, 5),
            onDone: () => ctx.refresh(),
          }),
      }, [text('追加 5 轮继续')])
    )
  );
  row.appendChild(
    registerWrite(
      h('button.btn.btn--sm', {
        type: 'button',
        title: '跑一轮复盘，把这次失败教给后面的任务。任务停在原地不动。',
        onClick: () =>
          attempt({
            label: '复盘这次失败',
            success: '复盘轮已启动，任务状态不变。',
            run: (write) => write.retroTask(task.id),
            onDone: () => ctx.refresh(),
          }),
      }, [text('复盘一次')])
    )
  );
  row.appendChild(
    registerWrite(
      h('button.btn.btn--sm', {
        type: 'button',
        onClick: () =>
          attempt({
            label: '从设计重跑',
            success: '保留任务文件与附件，设计循环从第 1 轮开始。',
            run: (write) => write.rerunFrom(task.id, 'design'),
            onDone: () => ctx.refresh(),
          }),
      }, [text('从设计重跑')])
    )
  );
  row.appendChild(
    registerWrite(
      h('button.btn.btn--sm', {
        type: 'button',
        onClick: () =>
          attempt({
            label: '从实现重跑',
            success: '设计保持不变，实现循环重新开始。',
            run: (write) => write.rerunFrom(task.id, 'implement'),
            onDone: () => ctx.refresh(),
          }),
      }, [text('从实现重跑')])
    )
  );
  row.appendChild(
    registerWrite(
      h('button.btn.btn--sm.btn--text', {
        type: 'button',
        onClick: () => confirmCancel(task, ctx),
      }, [text('取消任务')])
    )
  );
  card.appendChild(row);
}

// ---------------------------------------------------------------------------
// Documents and sessions
// ---------------------------------------------------------------------------

/** T-13's six produced files. Opening one is by name; Main resolves the path. */
function documentsCard(data) {
  const task = (data && data.task) || {};
  const documents = (data && data.documents) || [];
  const card = h('div.card.card--pad.col', [
    cardHead('产物', h('span.muted.small', { text: `docs/${task.slug || ''}/` })),
  ]);

  if (!documents.length) {
    card.appendChild(empty('任务还没有产出文档。第一个会是任务文件。'));
    return card;
  }

  const list = h('div.doclist');
  for (const doc of documents) {
    const row = h('div.docrow', {
      tabindex: '0',
      role: 'button',
      onClick: () => openDocument(task, doc),
      onKeyDown: activateOnKey,
    });
    row.appendChild(icon('doc'));
    const label = h('span', { text: doc.name });
    label.appendChild(h('small', { text: ` · ${doc.label}` }));
    row.appendChild(label);
    row.appendChild(h('small', { text: `${Math.max(1, Math.round((doc.size || 0) / 1024))} KB` }));
    list.appendChild(registerWrite(row));
  }
  card.appendChild(list);
  return card;
}

function openDocument(task, doc) {
  return attempt({
    label: `已在编辑器打开 ${doc.name}`,
    success: doc.path,
    run: (write) => write.open({ kind: 'document', taskId: task.id, name: doc.name }),
  });
}

/** T-14. One row per session; the log opens in a drawer, read on demand. */
function sessionsCard(data) {
  const sessions = (data && data.sessions) || [];
  const card = h('div.card.card--pad.col', [
    cardHead(
      '会话历史',
      h('span.muted.small', {
        text: sessions.length ? `${sessions.length} 次 · 点开看完整日志` : '还没有会话',
      })
    ),
  ]);

  if (!sessions.length) {
    card.appendChild(empty('任务还没有启动过任何 CLI 会话。'));
    return card;
  }

  const list = h('div.sesslist');
  // Newest first: the session a user wants is almost always the last one.
  const ordered = sessions.slice().sort((a, b) => String(b.started_at).localeCompare(String(a.started_at)));
  for (const session of ordered.slice(0, 8)) {
    const row = h('div.sess', {
      tabindex: '0',
      role: 'button',
      onClick: () => openLogDrawer(session),
      onKeyDown: activateOnKey,
    });
    row.appendChild(h('time', { text: labels.clock(session.started_at) }));
    const middle = h('span', [h('span.r', { text: `${session.label || ''} #${session.round}` })]);
    middle.appendChild(
      h('span.m', {
        text: ` · ${labels.runtimeLabel(session.runtime)} ${session.model}${
          session.ended_at ? ` · ${labels.duration(session.started_at, session.ended_at)}` : ''
        }`,
      })
    );
    row.appendChild(middle);
    row.appendChild(sessionOutcomeTag(session));
    list.appendChild(row);
  }
  card.appendChild(list);
  return card;
}

function sessionOutcomeTag(session) {
  const lifecycle = session.lifecycle || {};
  if (session.running) return spinnerTag('运行中', 'solid-teal');
  switch (lifecycle.state) {
    case 'exited':
      return lifecycle.exit_code === 0
        ? tag('正常结束', 'soft-green')
        : tag(`退出码 ${lifecycle.exit_code}`, 'soft-red');
    case 'killed':
      return tag('被停止', 'soft-yellow');
    case 'vanished':
      return tag('进程消失', 'soft-red');
    default:
      return tag(lifecycle.state || '未知', 'outlined');
  }
}

function openLogDrawer(session) {
  const body = h('div', [h('div.quiet', { text: '正在读取日志…' })]);
  openDrawer({
    title: `${session.label || '会话'} #${session.round}`,
    body: [body],
    footer: [closeButton('关闭')],
  });

  read('sessionLog', session.id)
    .then((payload) => {
      body.replaceChildren();
      const head = h('div.row.mb-12', [
        tag(`${labels.runtimeLabel(session.runtime)} · ${session.model}${session.effort ? ` · ${session.effort}` : ''}`, 'soft-purple'),
        tag(
          `${labels.clock(session.started_at)}${session.ended_at ? ` → ${labels.clock(session.ended_at)}` : ''}`,
          'outlined'
        ),
      ]);
      if (payload.truncated) head.appendChild(tag('只显示末尾 256 KB', 'soft-yellow'));
      body.appendChild(head);
      body.appendChild(
        h('div.quiet.mb-12', {
          text: `技能：${(session.skills || []).join(' · ') || '未绑定'} · 日志：${payload.path || ''}`,
        })
      );
      // The log is agent output. `.log` sets textContent, never markup.
      body.appendChild(h('div.log', { text: payload.log || '（日志是空的）' }));
    })
    .catch((err) => {
      body.replaceChildren();
      body.appendChild(empty(`读不到这次会话的日志：${err && err.message ? err.message : err}`));
    });
}

function openFilesDrawer(changes) {
  const stack = h('div.stack.stack--tight');
  for (const file of changes.files || []) {
    const row = h('div.filerow', [h('span', { text: file.path })]);
    const counts = h('span', [h('span.add', { text: `+${file.added}` })]);
    counts.appendChild(h('span.del', { text: `−${file.deleted}` }));
    row.appendChild(counts);
    stack.appendChild(row);
  }

  const subjects = h('div.quiet');
  for (const subject of changes.subjects || []) {
    subjects.appendChild(text(subject));
    subjects.appendChild(h('br'));
  }

  openDrawer({
    title: '变更文件',
    body: [
      h('div.row.mb-12', [
        tag(`${changes.branch} → ${changes.into}`, 'soft-teal'),
        tag(`${changes.commits} commits`, 'outlined'),
        tag(`${(changes.files || []).length} 文件 · +${changes.total_added} −${changes.total_deleted}`, 'outlined'),
      ]),
      stack.childNodes.length ? stack : empty('这个分支没有改动任何文件。'),
      h('div.sec.sec--gap', [h('h3', { text: '提交' })]),
      subjects,
    ],
    footer: [closeButton('关闭')],
  });
}

/**
 * The merge confirmation, reachable from the dashboard and the project page
 * as well as from the panel. It reads the change summary itself rather than
 * accepting one from the caller: the caller's copy may be seconds old, and
 * seconds are enough for the main worktree to become dirty.
 */
export async function openMergeModal(taskId, ctx) {
  const changes = await readOr(null, 'taskChanges', taskId);
  if (!changes || !changes.available) {
    openModal({
      title: '合并',
      lead: '现在读不到这个任务的变更。',
      body: [empty('分支可能已经被清理，或者 git 还没有回答。刷新一次再试。')],
      footer: [closeButton('知道了')],
    });
    return;
  }

  const included = (changes.pending_decisions || {}).included || 0;
  const mergeButton = registerWrite(
    h('button.btn.btn--yellow.ml-auto', {
      type: 'button',
      onClick: () =>
        attempt({
          label: included ? '已回到实现轮' : `已合并到 ${changes.into}`,
          success: included
            ? `${included} 个 Backlog 项会先做完，做完后回到这里。`
            : 'worktree 与分支已清理，任务完成。',
          run: (write) => write.mergeTask(taskId),
          onDone: () => {
            close();
            return ctx.refresh();
          },
        }),
    }, [text(included ? `先做 ${included} 个 Backlog 项` : '合并')])
  );
  if (!changes.mergeable) {
    mergeButton.disabled = true;
    mergeButton.title = blockedText(changes.blocked_by);
  }

  openModal({
    title: `合并到 ${changes.into}`,
    lead: '合并由你点，不会自动发生。',
    body: [
      h('dl.bindlist', [
        h('dt', { text: '分支' }),
        h('dd', { text: `${changes.branch} → ${changes.into}` }),
        h('dt', { text: '提交' }),
        h('dd', { text: `${changes.commits} commits · ${(changes.files || []).length} 文件 · +${changes.total_added} −${changes.total_deleted}` }),
        h('dt', { text: '前置条件' }),
        h('dd', { text: changes.mergeable ? '满足 · 主工作树干净且分支在最新默认分支之上' : blockedText(changes.blocked_by) }),
        h('dt', { text: '合并后' }),
        h('dd', { text: '删除 worktree 与分支 · docs 留在默认分支' }),
      ]),
      h('div.row.mt-12', [
        h('button.btn.btn--sm', { type: 'button', onClick: () => openFilesDrawer(changes) }, [
          text('看文件列表'),
        ]),
      ]),
    ],
    footer: [closeButton('稍后'), mergeButton],
  });
}
