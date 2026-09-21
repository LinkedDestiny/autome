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
import { read, attempt, registerWrite, coreMessage } from '../lib/api.js';
import { openModal, openDrawer, closeButton, close } from '../lib/overlay.js';
import { notify } from '../lib/notify.js';

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
      h('span.pagehead__sub', { text: '项目 = 一个目录 · 单个仓库会自动 init · 一堆独立仓库按工作区收编 · 任务都在项目下' }),
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
      // A directory holding several repositories is not something Autome
      // decides about on its own: `git init` over one of those quietly makes
      // every member a gitlink, and the first task then dies committing its
      // own documents. Main holds the path and asks.
      if (result.pending) return askAboutWorkspace(result.probe, ctx);
      await landProject(result, ctx);
    },
  });
}

async function landProject(result, ctx) {
  const project = (result && result.project) || result;
  if (project && project.id) ctx.navigate('project', { projectId: project.id });
  else await ctx.refresh();
}

/**
 * The one question a multi-repository directory raises: is this a workspace,
 * and if so which member holds the task documents?
 *
 * Phrased as two real options rather than a warning with an OK button,
 * because both answers are legitimate — someone may genuinely want a
 * repository that contains repositories — and only the user knows which they
 * meant.
 */
function askAboutWorkspace(probe, ctx) {
  const members = probe.members || [];
  const select = h('select.select');
  for (const member of members) {
    const option = h('option', { value: member.name, text: member.name });
    if (member.name === probe.suggested_docs_repo) option.selected = true;
    select.appendChild(option);
  }

  const confirm = (workspace) => async () => {
    close();
    await attempt({
      label: workspace ? '添加工作区' : '添加项目',
      success: false,
      run: (write) =>
        write.confirmProject({ workspace, docsRepo: workspace ? select.value : undefined }),
      onDone: (result) => landProject(result, ctx),
    });
  };

  // Which answer is offered first follows the probe. A directory that is
  // already a repository *and* holds several is asked about too, but there the
  // safe default is to leave it as the repository it already is.
  const workspaceFirst = Boolean(probe.suggest_workspace);
  const asWorkspace = h(
    `button.btn${workspaceFirst ? '.btn--primary' : ''}`,
    { type: 'button', onClick: confirm(true) },
    [workspaceFirst ? icon('check') : null, text('按工作区添加')]
  );
  const asRepo = h(
    `button.btn${workspaceFirst ? '' : '.btn--primary'}`,
    { type: 'button', onClick: confirm(false) },
    [workspaceFirst ? null : icon('check'), text('当成单个仓库')]
  );

  openModal({
    title: '这个目录里有好几个仓库',
    lead: probe.path,
    body: [
      h('p.quiet', {
        text:
          `找到 ${members.length} 个各自独立的仓库：${members.map((m) => m.name).join('、')}。` +
          '按工作区添加的话，Autome 不会在这个目录上执行 git init，也不会在它的根目录提交任何东西。',
      }),
      probe.is_repo_root
        ? h('p.quiet.mt-6', {
            text:
              '这个目录本身也已经是一个 Git 仓库了。如果那个仓库是 Autome 之前自己建的，' +
              '它会把每个子仓库记成 gitlink，任务文档提交不进去——那种情况该按工作区添加，' +
              '并把外层那个仓库删掉。',
          })
        : null,
      h('div.row.mt-8', [h('label', { text: '任务文档放进哪个仓库' }), select]),
      h('p.quiet.mt-6', {
        text: '文档会落在那个仓库的 autome/<任务>/ 下，可以之后在 .autome/config.toml 里改。',
      }),
    ],
    footer: workspaceFirst
      ? [closeButton('取消'), asRepo, asWorkspace]
      : [closeButton('取消'), asWorkspace, asRepo],
  });
}

/**
 * C-02's wizard.
 *
 * Steps 1 and 2 have already happened by the time a project exists. The three
 * that remain each need something different from the user, so the modal is
 * step-aware rather than a static list with one "next" button:
 *
 *  - **3** runs Claude Code in a visible terminal. The user watches it there;
 *    this screen only starts it and says where to look.
 *  - **4** is the confirm-and-edit step. The two artefacts are read through
 *    the core — the renderer has no filesystem — and edited in a drawer,
 *    because two documents do not fit a modal at 944px.
 *  - **5** points at the routing graph, which is where Loop configuration
 *    actually lives; duplicating it here would give two places to change one
 *    thing.
 *
 * Skipping is offered at every step, because the whole wizard is optional: a
 * skipped project uses the global defaults and its first task writes its own
 * profile (C-02).
 */
function openOnboarding(project, ctx) {
  const step = (project.onboarding && project.onboarding.step) || 3;
  const steps = [
    ['选择目录', project.path || ''],
    ['初始化 .autome', '脚手架已写入 · init commit 已提交'],
    ['Claude Code 起草画像与 AGENTS.md', '在后台运行，日志可随时打开'],
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

  const advance = (label, success) =>
    registerWrite(
      h('button.btn.btn--yellow.ml-auto', {
        type: 'button',
        onClick: () =>
          attempt({
            label,
            success,
            run: (write) => write.advanceOnboarding(project.id),
            onDone: () => {
              close();
              return ctx.refresh();
            },
          }),
      }, [text(label)])
    );

  // The step-specific action. Everything else about the modal is the same.
  let action;
  if (step === 3) {
    body.push(
      h('p.quiet.mt-8', {
        text:
          '起草会在可见终端里运行，空目录时 Claude 会先问你几个关于项目定位的问题。' +
          '会话结束后回到这里点「下一步」。',
      })
    );
    action = [
      registerWrite(
        h('button.modal__btn', {
          type: 'button',
          onClick: () =>
            attempt({
              label: '已开始起草',
              // The notification is written in onDone, where the session id is
              // available, so the generic success line is suppressed.
              success: false,
              run: (write) => write.runOnboarding(project.id),
              onDone: (result) =>
                notify(
                  'info',
                  '已在终端中开始起草',
                  `会话 ${result && result.session_id ? result.session_id : ''}`.trim()
                ),
            }),
        }, [text('运行起草会话')])
      ),
      advance('下一步', '进入「确认产物」。'),
    ];
  } else if (step === 4) {
    action = [
      h('button.modal__btn', {
        type: 'button',
        onClick: () => openArtefactEditor(project, ctx),
      }, [text('查看并编辑产物')]),
      advance('确认，下一步', '进入「Loop 配置」。'),
    ];
  } else {
    body.push(
      h('p.quiet.mt-8', {
        text: '在路由图里为这个项目设置五个角色；未覆盖的字段继续跟随全局默认。',
      })
    );
    action = [
      h('button.modal__btn', {
        type: 'button',
        onClick: () => {
          close();
          ctx.navigate('routing', { projectId: project.id });
        },
      }, [text('去路由图')]),
      advance('完成 Onboarding', '这个项目已就绪。'),
    ];
  }

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
      ...action,
    ],
  });
}

/**
 * Step 4's editor (C-02). Reads both artefacts through the core and saves each
 * one separately, so a failure on one does not lose edits to the other.
 *
 * The renderer never names the path: it saves whichever path the core told it
 * about, and the core accepts exactly two (see `ONBOARDING_FILES` in
 * dispatch.rs, and the same list again in the write gate).
 */
async function openArtefactEditor(project, ctx) {
  let payload;
  try {
    payload = await read('onboardingArtefacts', project.id);
  } catch (err) {
    notify('error', '读取产物失败', coreMessage(err));
    return;
  }
  const files = (payload && payload.files) || [];
  if (!files.length) {
    notify('info', '还没有产物', '先运行第 3 步的起草会话。');
    return;
  }

  const editors = files.map((file) => {
    // A textarea's initial content is its text node, not a `value`
    // attribute — setting the attribute leaves the box empty and the user
    // would save an emptied file over their profile.
    const area = h('textarea.artefact__text', {
      spellcheck: 'false',
      text: file.content || '',
    });
    const status = h('span.quiet');
    const save = registerWrite(
      h('button.btn.btn--sm', {
        type: 'button',
        onClick: () =>
          attempt({
            label: `已保存 ${file.path}`,
            success: false,
            run: (write) => write.saveOnboardingFile(project.id, file.path, area.value),
            onDone: () => {
              status.textContent = '已保存';
            },
          }),
      }, [text('保存')])
    );
    // Any edit invalidates the "saved" note, so it can never describe an
    // older version of what is on screen.
    area.addEventListener('input', () => {
      status.textContent = '未保存';
    });
    return h('div.artefact', [
      h('div.artefact__head', [
        h('b', { text: file.path }),
        file.exists ? tag('已生成', 'soft-green') : tag('尚未生成', 'dashed-brown'),
        h('span.ml-auto', [status]),
        save,
      ]),
      area,
    ]);
  });

  openDrawer({
    title: '确认 Onboarding 产物',
    body: [
      h('p.quiet', {
        text:
          '这两个文件是每一轮会话都会读到的项目背景与规范。改完保存即可；' +
          '它们是仓库里的普通文件，之后也可以直接用编辑器改。',
      }),
      ...editors,
    ],
    footer: [closeButton('关闭')],
    onClose: () => ctx.refresh(),
  });
}

// Test seams. The wizard and its editor are reached through a tile click in
// normal use, which needs a rendered project list and a live read; exporting
// them lets the harness exercise each step directly. Named so a reader can see
// at a glance that nothing in the app calls them.
export const __testOpenOnboarding = openOnboarding;
export const __testOpenArtefactEditor = openArtefactEditor;
