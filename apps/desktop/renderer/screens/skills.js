// 技能 — requirements S-01 … S-06, U-10.
//
// A read-only inventory. S-06 is the shape of this screen: Autome lists what
// it found and can open the directory, and that is all — no creating, no
// editing, no skill store.
//
// The one thing the screen does write is the binding (S-03), and it writes it
// through `config.set_role`, because a binding *is* a role's skill list. That
// is why the binding drawer edits a role's whole list rather than a
// skill-to-role table: there is no such table in the core, and inventing one
// in the renderer would put the two out of step.
//
// S-04's conflict comes from the core as `conflicts` on each skill, so a
// binding to a CLI that cannot see the skill goes red without this file
// re-deriving visibility.

import { h, icon, text, reveal, tag, empty } from '../lib/dom.js';
import { read, readOr, attempt, registerWrite } from '../lib/api.js';
import { openDrawer, closeButton, close } from '../lib/overlay.js';
import * as labels from '../lib/labels.js';

export const id = 'skills';
export const nav = 'skills';

export async function load(ctx) {
  const projectId = ctx.params.projectId || null;
  const inventory = await read('listSkills', projectId || undefined);
  // The binding drawer needs each role's current skill list to write a
  // complete one back.
  const config = await readOr(null, 'getConfig', projectId || undefined);
  let projectName = null;
  if (projectId) {
    const list = await readOr({ projects: [] }, 'listProjects');
    const entry = (list.projects || []).find((p) => p.project && p.project.id === projectId);
    projectName = entry ? entry.project.display_name : projectId;
  }
  return { ...inventory, config, projectId, projectName };
}

export function render(host, data, ctx) {
  const screen = h('div.screen.active', { 'data-screen': 'skills' });
  const skills = (data && data.skills) || [];
  const projectId = data.projectId || null;

  screen.appendChild(
    h('div.pagehead', [
      h('h1.ribbon.ribbon--purple', [h('span.ribbon__front', { text: '技能' })]),
      h('span.pagehead__sub', {
        text: '只读盘点 · Skill 随仓库走 · 绑定 = 该角色必须使用 · 未绑定的角色从全部可见技能自选',
      }),
      h('div.pagehead__actions', [scopeToggle(ctx, projectId, data)]),
    ])
  );

  if (!skills.length) {
    screen.appendChild(
      empty(
        projectId
          ? '这个仓库的 .claude/skills 与 .agents/skills 下没有技能，全局目录也没有。'
          : '~/.claude/skills 与 ~/.agents/skills 下没有技能。'
      )
    );
    host.appendChild(screen);
    return;
  }

  const grid = h('div.cardgrid.cardgrid--3');
  skills.forEach((skill, index) => grid.appendChild(reveal(skillCard(skill, data, ctx), index + 1)));
  screen.appendChild(grid);
  host.appendChild(screen);
}

function scopeToggle(ctx, projectId, data) {
  const toggle = h('span.scopetoggle', { role: 'tablist' });
  toggle.appendChild(
    h('button', {
      type: 'button',
      class: projectId ? '' : 'active',
      onClick: () => ctx.navigate('skills'),
    }, [text('全部')])
  );
  if (projectId) {
    toggle.appendChild(
      h('button', { type: 'button', class: 'active' }, [
        text(`项目：${data.projectName || projectId}`),
      ])
    );
  }
  return toggle;
}

function skillCard(skill, data, ctx) {
  const conflicts = skill.conflicts || [];
  const bound = skill.bound_roles || [];
  const card = h(`div.card.skillcard${conflicts.length ? '.card--pattern.card--pattern-red' : ''}`);

  const scopeTag = tag(
    skill.project_scoped ? `项目 · ${data.projectName || '当前仓库'}` : '全局',
    skill.project_scoped ? 'soft-blue' : 'soft-brown'
  );
  scopeTag.classList.add('ml-auto');
  card.appendChild(h('div.row', [h('span.skillcard__n', { text: skill.name }), scopeTag]));

  // S-01: where it was found. Paths, not a summary — the user opens them.
  card.appendChild(
    h('div.skillcard__src', {
      text: (skill.sources || []).map((s) => s.path).join(' · ') || '（没有记录来源）',
    })
  );

  // S-02: visibility per CLI.
  const visible = skill.visible_to || [];
  card.appendChild(
    h('div.skillcard__row', [
      h('span.k', { text: '可见' }),
      visible.includes('claude') ? tag('Claude ✓', 'soft-green') : tag('Claude —', 'outlined'),
      visible.includes('codex') ? tag('Codex ✓', 'soft-green') : tag('Codex —', 'outlined'),
    ])
  );

  const bindRow = h('div.skillcard__row', [h('span.k', { text: '绑定' })]);
  if (!bound.length) {
    bindRow.appendChild(tag('未绑定 · 各角色自选', 'outlined'));
  } else {
    for (const role of bound) {
      const conflicted = conflicts.includes(role);
      bindRow.appendChild(
        conflicted
          ? tag(`${labels.roleLabel(role)} · 该 CLI 看不到`, 'solid-red', 'warn')
          : tag(labels.roleLabel(role), 'solid-teal')
      );
    }
  }
  card.appendChild(bindRow);

  const actions = h('div.row.mt-auto.pt-6.gap-6');
  actions.appendChild(
    registerWrite(
      h('button.btn.btn--sm', {
        type: 'button',
        onClick: () =>
          attempt({
            label: `已打开 ${skill.name}`,
            success: (skill.sources || [])[0] ? skill.sources[0].path : undefined,
            run: (write) =>
              write.open({ kind: 'skill', projectId: data.projectId || undefined, name: skill.name }),
          }),
      }, [text('打开目录')])
    )
  );
  actions.appendChild(
    registerWrite(
      h(`button.btn.btn--sm${conflicts.length ? '.btn--primary' : ''}`, {
        type: 'button',
        onClick: () => openBindDrawer(skill, data, ctx),
      }, [text(conflicts.length ? '修正绑定' : '修改绑定')])
    )
  );
  card.appendChild(actions);
  return card;
}

/**
 * S-03/S-04. One checkbox per role; a role whose CLI cannot see the skill
 * cannot be checked, and if it already is, the panel says so and refuses to
 * save rather than quietly unchecking it — the user chose that binding and is
 * entitled to decide whether to drop it or move the skill.
 */
function openBindDrawer(skill, data, ctx) {
  const roles = ((data.config && data.config.resolved) || {}).roles || [];
  const visible = skill.visible_to || [];
  const selected = new Set(skill.bound_roles || []);
  const projectId = data.projectId || null;

  const list = h('div.stack.gap-8');
  const boxes = [];
  for (const resolved of roles) {
    const role = resolved.role;
    const runtime = resolved.config.runtime;
    const canSee = visible.includes(runtime);
    const label = h('label.cbx', { tabindex: '0' });
    const box = h('span.cbx__box');
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('viewBox', '0 0 12 11');
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', 'M1 5.5 L4.5 9 L11 1.5');
    svg.appendChild(path);
    box.appendChild(svg);
    label.appendChild(box);

    const caption = h('span', { text: `${labels.roleLabel(role)} · ${labels.runtimeLabel(runtime)}` });
    if (!canSee) caption.appendChild(tag('看不到此技能', 'soft-red', 'warn'));
    label.appendChild(caption);

    if (selected.has(role)) label.classList.add('checked');
    const toggle = () => {
      if (selected.has(role)) selected.delete(role);
      else selected.add(role);
      label.classList.toggle('checked', selected.has(role));
      refresh();
    };
    label.addEventListener('click', (event) => {
      event.preventDefault();
      toggle();
    });
    label.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        toggle();
      }
    });
    boxes.push({ role, runtime, canSee });
    list.appendChild(label);
  }

  const impact = h('div.impact.impact--error.mt-12');
  const impactText = h('div');
  impact.appendChild(icon('warn'));
  impact.appendChild(impactText);

  const save = registerWrite(
    h('button.btn.btn--yellow.ml-auto', {
      type: 'button',
      onClick: () => saveBindings(skill, roles, selected, projectId, ctx),
    }, [text('保存')])
  );

  function conflictingRoles() {
    return boxes.filter((b) => selected.has(b.role) && !b.canSee);
  }

  function refresh() {
    const bad = conflictingRoles();
    impactText.replaceChildren();
    if (!bad.length) {
      impact.classList.add('hide');
      save.disabled = false;
      save.removeAttribute('title');
      return;
    }
    impact.classList.remove('hide');
    const first = bad[0];
    impactText.appendChild(
      text(
        `${labels.roleLabel(first.role)}角色当前用 ${labels.runtimeLabel(first.runtime)}，而 ${skill.name} 不在它的技能目录里。取消勾选，或者把它也放进对应目录。`
      )
    );
    save.disabled = true;
    save.title = '绑定到看不见此技能的 CLI，保存被阻止';
  }
  refresh();

  openDrawer({
    title: skill.name,
    body: [
      h('div.row.mb-12', [
        tag(skill.project_scoped ? '项目' : '全局', 'soft-brown'),
        visible.includes('claude') ? tag('Claude ✓', 'soft-green') : tag('Claude —', 'outlined'),
        visible.includes('codex') ? tag('Codex ✓', 'soft-green') : tag('Codex —', 'outlined'),
      ]),
      h('div.quiet.mb-12', {
        text: '勾选的角色启动时必须使用这个技能。角色所用 CLI 看不到它时不能勾选。绑定关系存在项目的 .autome/ 里，随仓库提交。',
      }),
      list,
      impact,
    ],
    footer: [closeButton('取消'), save],
  });
}

/**
 * A binding change rewrites the affected roles' skill lists. Only the roles
 * whose membership actually changed are written: rewriting all five would
 * turn a global-scope role into a project override for no reason (C-07).
 */
async function saveBindings(skill, roles, selected, projectId, ctx) {
  for (const resolved of roles) {
    const role = resolved.role;
    const current = (resolved.config.skills || []).slice();
    const has = current.includes(skill.name);
    const want = selected.has(role);
    if (has === want) continue;
    const next = want ? [...current, skill.name] : current.filter((s) => s !== skill.name);
    const result = await attempt({
      label: `已更新 ${labels.roleLabel(role)} 的绑定`,
      success: want ? `${skill.name} 现在是必须使用的技能。` : `${skill.name} 不再绑定给这个角色。`,
      run: (write) => write.setRole({ projectId: projectId || undefined, role, skills: next }),
    });
    if (!result.ok) break;
  }
  close();
  await ctx.refresh();
}
