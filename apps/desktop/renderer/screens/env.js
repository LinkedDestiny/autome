// 本地环境 — requirements E-01 … E-05, U-09.
//
// Exactly four components, because E-01 says exactly four. Each card shows
// what the probe found — present, version, path — and the two CLIs also show
// their login state. Nothing here is inferred: if the probe could not tell
// (`Login::Unknown`), the card says the state is unknown rather than picking
// the more likely of "logged in" and "expired", because the two have different
// fixes and guessing sends the user to the wrong one.
//
// Install and login both run in a visible terminal (E-02, E-03). The renderer
// never sees the command; Main asks the core for the recipe and runs it.

import { h, icon, text, reveal, tag, cardHead, sectionHead } from '../lib/dom.js';
import { read, readOr, attempt, registerWrite } from '../lib/api.js';
import { notify } from '../lib/notify.js';
import * as labels from '../lib/labels.js';

export const id = 'env';
export const nav = 'env';

export async function load() {
  const env = await read('environment');
  // The roles a component serves come from the resolved routing, not from a
  // hard-coded table: a user who moved 审计 to Claude Code should see that.
  const config = await readOr(null, 'getConfig');
  return { ...env, config };
}

export function render(host, data, ctx) {
  const screen = h('div.screen.active', { 'data-screen': 'env' });
  const environment = (data && data.environment) || {};
  const components = environment.components || [];
  // The probe runs on its own thread and takes tens of seconds, because it
  // runs four external programs and the CLIs have moved their login flags
  // around. Saying so beats four cards that look like findings.
  const probed = data && data.probed !== false;
  const prerequisites = (data && data.prerequisites) || {};

  screen.appendChild(
    h('div.pagehead', [
      h('h1.ribbon.ribbon--orange', [h('span.ribbon__front', { text: '本地环境' })]),
      h('span.pagehead__sub', {
        text: probed
          ? `只检测四样东西 · Homebrew ${prerequisites.brew ? '✓' : '缺失'} · npm ${prerequisites.npm ? '✓' : '缺失'} · 最近检测 ${labels.clock(environment.checked_at) || '—'}`
          : '只检测四样东西 · 正在检测…（每项最多几秒，检测期间不会挡住其它操作）',
      }),
      h('div.pagehead__actions', [
        registerWrite(
          h('button.btn', {
            type: 'button',
            onClick: (event) => recheck(event.currentTarget, ctx),
          }, [icon('search'), text('重新检测')])
        ),
      ]),
    ])
  );

  const grid = h('div.cardgrid.cardgrid--4');
  for (const [index, key] of labels.COMPONENT_ORDER.entries()) {
    const status = components.find((c) => c.component === key);
    grid.appendChild(reveal(componentCard(key, status, ctx), index + 1));
  }
  screen.appendChild(grid);

  screen.appendChild(sectionHead('被谁用到', '按当前生效的角色路由'));
  screen.appendChild(usageGrid(data));

  host.appendChild(screen);
}

function componentCard(key, status, ctx) {
  const name = labels.COMPONENT_LABELS[key] || key;

  if (!status) {
    const card = h('div.card.envcard.card--dashed');
    card.appendChild(h('div.envcard__n', { text: name }));
    card.appendChild(h('div.envcard__v', { text: '这次探测没有返回这一项' }));
    return card;
  }

  const login = status.login || { login: 'not_applicable' };
  const missing = !status.present;
  const expired = login.login === 'expired';
  const unknownLogin = login.login === 'unknown';
  const pattern = missing ? 'card--pattern card--pattern-yellow' : expired ? 'card--pattern card--pattern-red' : '';
  const card = h(`div.card.envcard${pattern ? `.${pattern.split(' ').join('.')}` : ''}`);

  const corner = missing
    ? tag('缺失', 'solid-yellow')
    : expired
      ? tag('登录失效', 'solid-red')
      : unknownLogin
        ? tag('登录未知', 'solid-yellow')
        : tag('就绪', 'solid-green');
  corner.classList.add('corner');
  card.appendChild(corner);

  card.appendChild(h('div.envcard__n', { text: name }));
  card.appendChild(
    h('div.envcard__v', {
      text: missing
        ? '未安装'
        : [status.version, status.path].filter(Boolean).join(' · ') || '已安装',
    })
  );

  const loginText = labels.loginLabel(login);
  if (loginText) {
    const dotClass = expired ? 'dotst--bad' : unknownLogin ? 'dotst--warn' : 'dotst--ok';
    card.appendChild(
      h('div.envcard__login', [h(`span.dotst.${dotClass}`, { text: loginText })])
    );
  }

  const actions = h('div.row.mt-10');
  if (missing) {
    actions.appendChild(
      registerWrite(
        h('button.btn.btn--sm.btn--primary', {
          type: 'button',
          onClick: () => install(key, name, ctx),
        }, [icon('plus'), text('一键安装')])
      )
    );
  } else if (expired || unknownLogin) {
    actions.appendChild(
      registerWrite(
        h('button.btn.btn--sm.btn--primary', {
          type: 'button',
          onClick: () =>
            attempt({
              label: `已打开终端 · ${name} 登录`,
              success: '登录完成后回到 Autome，会自动重新检测。',
              run: (write) => write.login(key),
            }),
        }, [icon('term'), text('去终端登录')])
      )
    );
  }
  if (actions.childNodes.length) card.appendChild(actions);

  card.appendChild(h('div.quiet.mt-8', { text: labels.COMPONENT_NOTE[key] || '' }));
  return card;
}

/**
 * E-02. The recipe — including whether Homebrew or npm is missing — is the
 * core's answer, so the modal states the command the user is about to see
 * rather than one this file guessed.
 */
async function install(component, name, ctx) {
  const recipe = await readOr(null, 'installRecipe', component);
  if (recipe && recipe.prerequisite_ok === false) {
    // E-02: Homebrew or npm missing is a different problem with a different
    // fix, so it is reported instead of running a command that will fail.
    notify('warning', `无法安装 ${name}`, `请先安装 ${recipe.recipe.prerequisite}`);
    return;
  }
  await attempt({
    label: `已打开终端 · 安装 ${name}`,
    success: recipe && recipe.recipe ? recipe.recipe.command : '命令在可见终端里执行，你能看到全过程。',
    run: (write) => write.install(component),
    onDone: () => ctx.refresh(),
  });
}

/** E-04's manual re-probe. */
function recheck(button, ctx) {
  button.classList.add('btn--loading');
  return attempt({
    label: '检测完成',
    success: false,
    run: (write) => write.detectEnvironment(),
    onDone: async () => {
      button.classList.remove('btn--loading');
      await ctx.refresh();
    },
  }).then((result) => {
    button.classList.remove('btn--loading');
    return result;
  });
}

function usageGrid(data) {
  const grid = h('div.cardgrid.cardgrid--3');
  const roles = ((data && data.config && data.config.resolved) || {}).roles || [];

  for (const [index, runtime] of ['claude', 'codex'].entries()) {
    const using = roles.filter((r) => r.config.runtime === runtime && r.config.enabled);
    const card = h('div.card.card--pad.col');
    const component = (data.environment && (data.environment.components || []).find((c) => c.component === runtime)) || null;
    const broken = component && (!component.present || (component.login && component.login.login === 'expired'));
    card.appendChild(
      cardHead(labels.RUNTIME_LABELS[runtime], broken ? tag('下一节点会失败', 'soft-red') : null)
    );
    const lines = h('div.roleline');
    if (!using.length) {
      lines.appendChild(h('b', { text: '—' }));
      lines.appendChild(h('span', { text: '当前没有启用的角色使用它' }));
    }
    for (const role of using) {
      lines.appendChild(h('b', { text: labels.roleLabel(role.role) }));
      lines.appendChild(h('span', { text: role.config.model }));
    }
    card.appendChild(lines);
    if (runtime === 'claude') {
      card.appendChild(h('div.quiet.mt-6', { text: '任务整理与 Onboarding 也固定使用 Claude Code。' }));
    }
    grid.appendChild(reveal(card, index + 5));
  }

  const howto = h('div.card.card--pad.col', [cardHead('安装方式')]);
  howto.appendChild(
    h('div.quiet', {
      text: '一键安装在可见终端里执行，你能看到过程与 sudo 提示。Claude Code 与 Codex 走 npm 全局安装或 Homebrew，iTerm2 走 Homebrew cask，Git 走 Xcode 命令行工具。Homebrew 或 npm 缺失时先提示安装它们。',
    })
  );
  grid.appendChild(reveal(howto, 7));
  return grid;
}
