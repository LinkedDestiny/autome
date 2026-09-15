// 仪表盘 — requirement U-02 / section 4.1.
//
// Monitoring only. The requirement is explicit about what is *not* here: no
// event stream, no telemetry, no statistics. Three regions, in this order:
// the environment banner (E-05, and only when there is a problem), everything
// waiting on the user across all projects, and everything running.
//
// `dashboard.get` returns `waiting` and `running` already split by the core
// (`TaskState::awaits_user` / `occupies_slot`), so the screen never re-derives
// which bucket a task belongs in. One classifier, in the core.

import {
  h, icon, text, reveal, tag, spinnerTag, progress, sectionHead, empty, activateOnKey,
} from '../lib/dom.js';
import { read } from '../lib/api.js';
import * as labels from '../lib/labels.js';
import { openMergeModal } from './task.js';

export const id = 'dash';
export const nav = 'dash';

export async function load() {
  return read('dashboard');
}

export function render(host, data, ctx) {
  const screen = h('div.screen.active', { 'data-screen': 'dash' });

  screen.appendChild(
    h('div.pagehead.pagehead--tight', [
      h('h1.ribbon.ribbon--teal', [h('span.ribbon__front', { text: '仪表盘' })]),
      h('span.pagehead__sub', { text: '监控所有项目里正在发生的事' }),
    ])
  );

  const banner = environmentBanner(data && data.environment, ctx);
  if (banner) screen.appendChild(banner);

  const waiting = (data && data.waiting) || [];
  const running = (data && data.running) || [];

  screen.appendChild(sectionHead('等待我', waiting.length ? `${waiting.length} 件 · 点卡片直达` : '没有需要你的事'));
  if (waiting.length) {
    const grid = h('div.cardgrid.cardgrid--4');
    waiting.forEach((entry, index) => grid.appendChild(reveal(waitingCard(entry, ctx), index + 1)));
    screen.appendChild(grid);
  } else {
    screen.appendChild(empty('所有任务都在自己跑。下一次需要你，是设计定稿或者待合并。'));
  }

  screen.appendChild(
    sectionHead('运行中', running.length ? `${running.length} 个任务` : '当前没有任务在跑')
  );
  if (running.length) {
    const grid = h('div.cardgrid.cardgrid--4');
    running.forEach((entry, index) =>
      grid.appendChild(reveal(runningCard(entry, ctx), waiting.length + index + 1))
    );
    screen.appendChild(grid);
  } else {
    screen.appendChild(empty('没有会话在运行。到项目里新建一个任务，它会自己开始。'));
  }

  host.appendChild(screen);
}

/**
 * E-05: red banner only when a CLI is missing or its login expired. `severity`
 * is the core's own judgement (`Environment::severity`), so the banner appears
 * exactly when the core would refuse to launch — never on a hunch here.
 */
function environmentBanner(environment, ctx) {
  if (!environment || environment.severity === 'ok') return null;
  const problems = environment.problems || [];
  const names = problems
    .map((p) => {
      const name = p.name || labels.COMPONENT_LABELS[p.component] || p.component;
      if (!p.present) return `${name} 缺失`;
      if (p.login && p.login.login === 'expired') return `${name} 登录已失效`;
      return `${name} 异常`;
    })
    .join(' · ');
  const bar = h('div.alertbar.reveal', [icon('warn'), text(names || '本地环境异常')]);
  bar.style.setProperty('--i', '0');
  bar.appendChild(text(' · 下一次启动这些角色会失败'));
  bar.appendChild(
    h('button.btn.btn--sm', { type: 'button', onClick: () => ctx.navigate('env') }, [text('去本地环境')])
  );
  return bar;
}

function waitingCard(entry, ctx) {
  const task = entry.task || {};
  const state = task.state || {};
  const node = labels.stateNode(state);
  const kind = waitingKind(state, node);

  const card = h(`div.card.card--pattern.card--pattern-${kind.color}.attn.card--link`, {
    tabindex: '0',
    role: 'button',
    onClick: kind.open ? () => kind.open(ctx, task, entry) : () => ctx.navigate('task', { taskId: task.id }),
    onKeyDown: activateOnKey,
  });

  const title = h('div.attn__t', { text: kind.title });
  title.appendChild(tag(`${entry.project_name || entry.project_id} · ${task.id}`, 'outlined'));

  const detail = h('div.attn__d', { text: kind.detail(task, entry) });

  const actions = h('div.attn__a', [
    h('button.btn.btn--sm.btn--primary', {
      type: 'button',
      onClick: (event) => {
        event.stopPropagation();
        if (kind.open) kind.open(ctx, task, entry);
        else ctx.navigate('task', { taskId: task.id });
      },
    }, [text(kind.action)]),
  ]);

  card.appendChild(h('div.attn__ico', [icon(kind.icon)]));
  card.appendChild(h('div', [title, detail, actions]));
  return card;
}

/** Which of the four "waiting" shapes this is (T-04, T-05, T-10). */
function waitingKind(state, node) {
  if (state.state === 'failed') {
    return {
      color: 'red',
      icon: 'warn',
      title: '停在失败上',
      action: '看选项',
      detail: (task) => `${task.title || task.request || ''} · ${labels.failureReason(state.reason)}`,
    };
  }
  if (node === 'await_merge') {
    return {
      color: 'teal',
      icon: 'branch',
      title: '待合并',
      action: '合并',
      open: (ctx, task) => openMergeModal(task.id, ctx),
      detail: (task) => `${task.title || task.request || ''} · 分支 ${task.branch || ''} · 合并由你点`,
    };
  }
  if (node === 'await_design_approval') {
    return {
      color: 'yellow',
      icon: 'hand',
      title: '批准设计',
      action: '看设计并批准',
      detail: (task, entry) => {
        const pending = entry.pending_decisions || {};
        const extra = pendingNote(pending);
        return `${task.title || task.request || ''} · 设计已定稿，等你定方向${extra ? ` · ${extra}` : ''}`;
      },
    };
  }
  return {
    color: 'yellow',
    icon: 'hand',
    title: `等你 · ${labels.nodeLabel(node)}`,
    action: '打开任务',
    detail: (task) => task.title || task.request || '',
  };
}

function pendingNote(pending) {
  const parts = [];
  if (pending.ruled) parts.push(`${pending.ruled} 条裁定待消费`);
  if (pending.included) parts.push(`${pending.included} 条纳入待消费`);
  return parts.join(' · ');
}

function runningCard(entry, ctx) {
  const task = entry.task || {};
  const session = entry.session;
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
  card.appendChild(
    h('div.row', [
      h('span.taskcard__id', { text: `${entry.project_name || entry.project_id} · ${task.id}` }),
      statusTag,
    ])
  );
  card.appendChild(h('div.taskcard__t', { text: task.title || task.request || task.slug || task.id }));
  card.appendChild(h('div.quiet', { text: sessionLine(session, task) }));
  card.appendChild(
    progress(
      flowIndex >= 0 ? (flowIndex + 1) / labels.FLOW.length : 0,
      flowIndex >= 0 ? `${flowIndex + 1} / ${labels.FLOW.length}` : '—',
      node === 'implement' ? null : 'blue'
    )
  );
  return card;
}

/**
 * What the running session is, in one line. When there is no session the line
 * says so rather than implying one: a task can occupy a slot while a core step
 * (rebase, cleanup) runs, and those have no CLI behind them.
 */
function sessionLine(session, task) {
  if (!session) {
    const elapsed = labels.duration(task.created_at);
    return elapsed ? `系统步骤运行中 · 已存在 ${elapsed}` : '系统步骤运行中';
  }
  const parts = [session.label || '', labels.runtimeLabel(session.runtime), session.model];
  const elapsed = labels.duration(session.started_at, session.ended_at);
  if (elapsed) parts.push(`会话 ${elapsed}`);
  return parts.filter(Boolean).join(' · ');
}
