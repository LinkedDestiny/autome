// 全局设置 — requirements C-08 / U-07.
//
// C-08 fixes what is adjustable globally: the five-role routing, the default
// project parallelism, the design round cap and the implementation budget
// factor. Sessions run headless and that is not a setting. Anything else on this
// screen would be a setting the requirement says does not exist.
//
// The routing itself is not edited here — it has a screen of its own, because
// a role's configuration is five fields with a cross-role constraint (C-06)
// and a summary card cannot express that. This card links there.

import {
  h, icon, text, reveal, tag, kvList, cardHead, activateOnKey,
} from '../lib/dom.js';
import { read, attempt, registerWrite } from '../lib/api.js';
import * as theme from '../lib/theme.js';
import * as labels from '../lib/labels.js';

export const id = 'settings';
export const nav = 'settings';

const PARALLEL_OPTIONS = [1, 2, 3, 4, 5];
const DESIGN_ROUND_OPTIONS = [5, 10, 15, 20, 30];
const BUDGET_FACTOR_OPTIONS = [2, 3, 4, 5, 6, 8, 10];

export async function load() {
  return read('getConfig');
}

export function render(host, data, ctx) {
  const screen = h('div.screen.active', { 'data-screen': 'settings' });

  screen.appendChild(
    h('div.pagehead', [
      h('h1.ribbon.ribbon--brown', [h('span.ribbon__front', { text: '全局设置' })]),
      h('span.pagehead__sub', {
        text: '所有项目的默认值 · 项目可逐角色覆盖 · 改动对下一个启动的节点生效',
      }),
    ])
  );

  screen.appendChild(
    h('div.cardgrid.cardgrid--2', [
      reveal(routingCard(data, ctx), 1),
      reveal(loopCard(data, ctx), 2),
      reveal(terminalCard(ctx), 3),
      reveal(appearanceCard(data, ctx), 4),
      reveal(aboutCard(), 5),
    ])
  );

  host.appendChild(screen);
}

function routingCard(data, ctx) {
  const resolved = (data && data.resolved) || {};
  const violations = (data && data.violations) || [];

  const card = h('div.card.card--pad.col.card--link', {
    tabindex: '0',
    role: 'button',
    onClick: () => ctx.navigate('routing'),
    onKeyDown: activateOnKey,
  });
  card.appendChild(
    cardHead(
      '默认角色路由',
      violations.length
        ? tag(`${violations.length} 项被阻止`, 'soft-red', 'warn')
        : tag('五个角色都有效', 'soft-green', 'check')
    )
  );

  const lines = h('div.roleline');
  for (const role of resolved.roles || []) {
    lines.appendChild(h('b', { text: labels.roleLabel(role.role) }));
    lines.appendChild(
      h('span', {
        text: `${labels.runtimeSummary(role.config)} · ${role.config.enabled ? '已启用' : '已关闭'}`,
      })
    );
  }
  card.appendChild(lines);
  // C-06 in one sentence, where the user is most likely to be about to break it.
  card.appendChild(
    h('div.quiet.mt-8', { text: '双模型纪律：评审 ≠ 设计、审计 ≠ 实现。相同时无法保存。' })
  );
  const cta = h('span.card__cta', { text: '打开路由图 ' });
  cta.appendChild(icon('arrow', 'ic--sm'));
  card.appendChild(cta);
  return card;
}

function loopCard(data, ctx) {
  const defaults = ((data && data.global) || {}).loop_defaults || {};
  const card = h('div.card.card--pad.col', [cardHead('Loop 默认值')]);

  card.appendChild(
    kvList([
      [
        '项目并行默认',
        row(
          numberSelect(PARALLEL_OPTIONS, defaults.parallel, (value) =>
            saveLoop({ parallel: value }, ctx)
          ),
          '上限 5 · 项目内各自控制'
        ),
      ],
      [
        '设计轮次上限',
        row(
          numberSelect(DESIGN_ROUND_OPTIONS, defaults.design_rounds, (value) =>
            saveLoop({ designRounds: value }, ctx)
          ),
          '耗尽后停下给你选项'
        ),
      ],
      [
        '实现预算系数',
        row(
          numberSelect(BUDGET_FACTOR_OPTIONS, defaults.budget_factor, (value) =>
            saveLoop({ budgetFactor: value }, ctx)
          ),
          'N = 系数 × 初始里程碑数'
        ),
      ],
    ])
  );
  card.appendChild(
    h('div.matrix-note', { text: '新建项目时复制这份默认值；已有项目只在未覆盖的字段上跟随。' })
  );
  return card;
}

function row(control, note) {
  return h('span.row', [control, h('span.muted.small', { text: note })]);
}

function numberSelect(options, value, onChange) {
  const select = h('select.select', {
    onChange: (event) => onChange(Number(event.target.value)),
  });
  const present = options.includes(value);
  // A value the core holds that is not one of our options is still the truth;
  // showing the list without it would silently misreport the setting.
  const list = present || value === undefined ? options : [value, ...options];
  for (const option of list) {
    const el = h('option', { value: String(option), text: String(option) });
    if (option === value) el.selected = true;
    select.appendChild(el);
  }
  return registerWrite(select);
}

function saveLoop(patch, ctx) {
  return attempt({
    label: '已保存 Loop 默认值',
    success: '对下一个启动的节点生效，正在跑的会话不打断。',
    run: (write) => write.setLoop(patch),
    onDone: () => ctx.refresh(),
  });
}

/** The appearance switch. `跟随系统` is the default and tracks macOS live. */
function appearanceCard(data, ctx) {
  const current = (((data && data.global) || {}).ui || {}).theme || 'system';
  const card = h('div.card.card--pad.col', [
    cardHead('外观', tag(labels.themeLabel(current), 'outlined')),
  ]);
  card.appendChild(
    h('div.quiet', {
      text: '跟随系统时，macOS 切换深浅色，应用立刻跟着切，不需要重启。',
    })
  );

  const pick = h('div.pick.mt-12');
  for (const value of theme.THEMES) {
    const pill = tag(labels.themeLabel(value), 'outlined');
    if (value === current) pill.classList.add('on');
    pill.setAttribute('role', 'button');
    pill.setAttribute('tabindex', '0');
    pill.addEventListener('keydown', activateOnKey);
    pill.addEventListener('click', () =>
      attempt({
        label: '外观已切换',
        success: false,
        run: (write) => write.setTheme(value),
        onDone: () => {
          // Painted here rather than waiting for the refresh below, so the
          // window changes on the click instead of a tick later.
          theme.apply(value);
          return ctx.refresh();
        },
      })
    );
    pick.appendChild(registerWrite(pill));
  }
  card.appendChild(pick);
  return card;
}

function terminalCard(ctx) {
  const card = h('div.card.card--pad.col', [cardHead('会话', tag('后台运行', 'outlined'))]);
  card.appendChild(
    h('div.quiet', {
      text: '任务会话与 Onboarding 在后台运行，不开终端窗口——一次 Loop 会起六个以上会话，每个都弹窗会一直打断你手上的事。CLI 的全部输出照样写进会话日志，任务面板里点开就能看，也可以 tail -f 跟实时输出。只有一键安装与登录命令会开 iTerm2 可见终端，因为那是你主动按的、需要看到 sudo 提示。',
    })
  );
  card.appendChild(
    h('div.row.mt-auto.pt-10', [
      h('button.btn.btn--sm', { type: 'button', onClick: () => ctx.navigate('env') }, [
        text('去本地环境检查'),
      ]),
    ])
  );
  return card;
}

function aboutCard() {
  const card = h('div.card.card--pad.col', [cardHead('关于')]);
  card.appendChild(
    kvList([
      ['版本', 'Autome 2.0.0'],
      ['全局配置', '~/.autome/'],
      ['项目配置', '各仓库 .autome/（随仓库提交）'],
      ['本机台账', '~/Library/Application Support/Autome/'],
    ])
  );
  return card;
}
