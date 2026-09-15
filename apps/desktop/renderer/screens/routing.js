// 路由图 — requirements C-04 … C-07, U-08.
//
// The Loop as a picture: two lanes, thirteen boxes, and edges drawn between
// them. Five boxes are roles and open a configuration drawer; the rest are
// grey fixed system steps or yellow human stops and are deliberately inert,
// because the requirement says they are not configurable and a box that looks
// clickable but is not is worse than one that looks fixed.
//
// C-06 is enforced here twice over, on purpose. The core refuses a colliding
// save, and this screen refuses to offer one: the draft is validated with the
// same rule as `config::validate` (evaluator's `runtime:model` must differ
// from its generator's, with both enabled), the offending boxes go red and
// 保存 is disabled. Duplicating the rule is the point — the user should see
// the collision while typing, not after a round trip — but the core stays the
// authority, and a refusal from it is still surfaced verbatim.

import { h, icon, text, tag, activateOnKey } from '../lib/dom.js';
import { read, readOr, attempt, registerWrite, resetWriteControls } from '../lib/api.js';
import { openDrawer, closeButton, close } from '../lib/overlay.js';
import * as labels from '../lib/labels.js';

export const id = 'routing';
export const nav = 'settings';

const EFFORT_OPTIONS = [
  ['', '默认 Effort'],
  ['low', 'low'],
  ['medium', 'medium'],
  ['high', 'high'],
];

/** `[from, to, options]`, lifted from the design document's edge table. */
const EDGES = [
  ['intake', 'plan'],
  ['plan', 'review'],
  ['review', 'adjudicate'],
  ['adjudicate', 'review', { rej: 1, via: 'bottom', d: 26, label: '复审' }],
  ['adjudicate', 'design_approval'],
  ['design_approval', 'plan', { rej: 1, via: 'top', d: 26, label: '驳回 + 意见' }],
  ['design_approval', 'impl', { gy: 40, t: 0.2, label: '批准' }],
  ['impl', 'audit'],
  ['audit', 'impl', { rej: 1, via: 'bottom', d: 26, label: 'reopen' }],
  ['audit', 'rebase'],
  ['rebase', 'impl', { rej: 1, via: 'top', d: 26, ex: 10, label: '冲突' }],
  ['rebase', 'merge_wait'],
  ['merge_wait', 'merge'],
  ['merge', 'cleanup'],
  ['cleanup', 'done'],
];

const FIXED_NODES = {
  intake: ['任务整理', '固定 · 调研仓库，生成任务文件', 'Claude Code'],
  rebase: ['rebase', '固定 · 到最新默认分支', '冲突 → 实现修'],
  merge: ['合并', '固定 · merge 到默认分支', '主工作树须干净'],
  cleanup: ['清理', '固定 · 删 worktree 与分支', 'docs 留在默认分支'],
};
const HUMAN_NODES = {
  design_approval: ['设计批准', '人工 · 唯一阻塞停顿', '批准 / 驳回'],
  merge_wait: ['待合并', '人工 · 你点合并', '文件列表 · 统计'],
};
const ROLE_NODES = {
  plan: ['设计', 'plan · 写设计文档与里程碑'],
  review: ['评审', 'review · 独立模型挑问题'],
  adjudicate: ['裁决', 'adjudicate · 逐条采纳或驳回'],
  impl: ['实现', 'impl · 推进里程碑至待审'],
  audit: ['审计', 'audit · 独立复验，reopen 或关闭'],
};

export async function load(ctx) {
  const projectId = ctx.params.projectId || null;
  const config = await read('getConfig', projectId || undefined);
  const skills = await readOr({ skills: [] }, 'listSkills', projectId || undefined);
  let projectName = null;
  if (projectId) {
    const list = await readOr({ projects: [] }, 'listProjects');
    const entry = (list.projects || []).find((p) => p.project && p.project.id === projectId);
    projectName = entry ? entry.project.display_name : projectId;
  }
  return { ...config, skills: skills.skills || [], projectId, projectName };
}

export function render(host, data, ctx) {
  const projectId = data.projectId || null;
  // The draft is the user's uncommitted intention. It starts as a copy of the
  // resolved view so an untouched screen saves nothing.
  const draft = {};
  for (const role of ((data.resolved || {}).roles) || []) {
    draft[role.role] = {
      enabled: role.config.enabled,
      runtime: role.config.runtime,
      model: role.config.model,
      effort: role.config.effort || null,
      skills: (role.config.skills || []).slice(),
      provenance: role.provenance,
    };
  }

  const paint = () => {
    // Re-rendering replaces every button; the connection registry must not
    // keep the detached ones.
    resetWriteControls();
    host.replaceChildren();
    host.appendChild(buildScreen(data, ctx, draft, paint, projectId));
    requestAnimationFrame(() => drawEdges(host));
  };
  paint();
}

function buildScreen(data, ctx, draft, paint, projectId) {
  const screen = h('div.screen.active', { 'data-screen': 'routing' });
  const localViolations = sameModelViolations(draft);
  const serverViolations = (data.violations || []).filter((v) => v.kind !== 'same_model');
  const blocking = [...localViolations, ...serverViolations];
  const dirty = changedRoles(data, draft);

  screen.appendChild(
    h('div.crumb', [
      projectId
        ? h('button', {
            type: 'button',
            onClick: () => ctx.navigate('project', { projectId }),
          }, [text(data.projectName || '项目')])
        : h('button', { type: 'button', onClick: () => ctx.navigate('settings') }, [text('全局设置')]),
      h('span.sep', { text: '›' }),
      h('b', { text: '路由图' }),
    ])
  );

  const saveButton = registerWrite(
    h('button.btn.btn--primary', {
      type: 'button',
      onClick: () => saveDraft(data, draft, ctx, projectId),
    }, [icon('check'), text(dirty.length ? `保存 ${dirty.length} 项改动` : '保存')])
  );
  if (localViolations.length || !dirty.length) {
    saveButton.disabled = true;
    saveButton.title = localViolations.length
      ? labels.violationText(localViolations[0])
      : '没有未保存的改动';
  }

  screen.appendChild(
    h('div.pagehead.pagehead--tight', [
      h('h1.ribbon.ribbon--purple', [h('span.ribbon__front', { text: '路由图' })]),
      h('span.pagehead__sub', {
        text: '五个角色可点开配置 · 灰色为固定系统步骤 · 黄色为人工停顿 · 实线通过 · 虚线驳回',
      }),
      h('div.pagehead__actions', [
        scopeToggle(ctx, projectId, data),
        h('button.btn', {
          type: 'button',
          disabled: !dirty.length,
          onClick: () => ctx.refresh(),
        }, [text('放弃更改')]),
        saveButton,
      ]),
    ])
  );

  for (const violation of blocking) {
    const bar = h('div.alertbar.mb-8', [icon('warn'), text(labels.violationText(violation))]);
    screen.appendChild(bar);
  }

  const card = h('div.card.card--pad');
  card.appendChild(graph(data, draft, localViolations, paint, ctx));
  card.appendChild(legend());
  screen.appendChild(card);
  return screen;
}

/** C-07's two scopes. The project option only exists when we are in one. */
function scopeToggle(ctx, projectId, data) {
  const toggle = h('span.scopetoggle', { role: 'tablist' });
  const global = h('button', {
    type: 'button',
    class: projectId ? '' : 'active',
    onClick: () => ctx.navigate('routing'),
  }, [text('全局默认')]);
  toggle.appendChild(global);
  if (projectId) {
    toggle.appendChild(
      h('button', { type: 'button', class: 'active' }, [text(`项目：${data.projectName || projectId}`)])
    );
  }
  return toggle;
}

// ---------------------------------------------------------------------------
// The graph
// ---------------------------------------------------------------------------

function graph(data, draft, localViolations, paint, ctx) {
  const wrap = h('div.lgraph', { id: 'loop-graph' });
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('class', 'lgraph__edges');
  svg.setAttribute('id', 'edge-layer');
  svg.appendChild(markerDefs());
  wrap.appendChild(svg);

  const badRoles = new Set(localViolations.flatMap(labels.violationRoles));
  for (const violation of data.violations || []) {
    for (const role of labels.violationRoles(violation)) badRoles.add(role);
  }

  const laneOne = h('div.lane.lane--7', { dataset: { lane: '1' } }, [
    h('span.lane__lbl', { text: `设计循环 · 最多 ${((data.resolved || {}).loop_defaults || {}).design_rounds ?? '—'} 轮` }),
    fixedNode('intake'),
    roleNode('plan', draft, badRoles, data, paint, ctx),
    roleNode('review', draft, badRoles, data, paint, ctx),
    roleNode('adjudicate', draft, badRoles, data, paint, ctx),
    humanNode('design_approval'),
  ]);

  const laneTwo = h('div.lane.lane--7', { dataset: { lane: '2' } }, [
    h('span.lane__lbl', { text: `实现循环 · 预算 ${((data.resolved || {}).loop_defaults || {}).budget_factor ?? '—'} × 里程碑` }),
    roleNode('impl', draft, badRoles, data, paint, ctx),
    roleNode('audit', draft, badRoles, data, paint, ctx),
    fixedNode('rebase'),
    humanNode('merge_wait'),
    fixedNode('merge'),
    fixedNode('cleanup'),
    endNode(),
  ]);

  wrap.appendChild(laneOne);
  wrap.appendChild(laneTwo);
  return wrap;
}

function markerDefs() {
  const defs = document.createElementNS('http://www.w3.org/2000/svg', 'defs');
  for (const [id, colour] of [['arrPass', '#19c8b9'], ['arrRej', '#e59266']]) {
    const marker = document.createElementNS('http://www.w3.org/2000/svg', 'marker');
    marker.setAttribute('id', id);
    marker.setAttribute('viewBox', '0 0 10 10');
    marker.setAttribute('refX', '9');
    marker.setAttribute('refY', '5');
    marker.setAttribute('markerWidth', '7');
    marker.setAttribute('markerHeight', '7');
    marker.setAttribute('orient', 'auto-start-reverse');
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', 'M0,0 L10,5 L0,10 z');
    path.setAttribute('fill', colour);
    marker.appendChild(path);
    defs.appendChild(marker);
  }
  return defs;
}

function fixedNode(key) {
  const [title, role, chip] = FIXED_NODES[key];
  const node = h('div.lnode.lnode--rust', { dataset: { node: key } }, [
    icon('lock'),
    h('div.lnode__id', { text: title }),
    h('div.lnode__role', { text: role }),
    h('div.lnode__cfg', [h('span.chip', { text: chip })]),
  ]);
  return node;
}

function humanNode(key) {
  const [title, role, chip] = HUMAN_NODES[key];
  return h('div.lnode.lnode--human', { dataset: { node: key } }, [
    icon('hand'),
    h('div.lnode__id', { text: title }),
    h('div.lnode__role', { text: role }),
    h('div.lnode__cfg', [h('span.chip', { text: chip })]),
  ]);
}

function endNode() {
  return h('div.lnode.lnode--end', { dataset: { node: 'done' } }, [
    h('div.lnode__id', { text: '完成' }),
    h('div.lnode__role', { text: '已合并进默认分支' }),
  ]);
}

function roleNode(role, draft, badRoles, data, paint, ctx) {
  const config = draft[role];
  const [title, subtitle] = ROLE_NODES[role];
  const bad = badRoles.has(role);
  const runtimeClass = role === 'impl' ? 'lnode--impl' : config && config.runtime === 'codex' ? 'lnode--codex' : 'lnode--claude';

  const node = h(`div.lnode.lnode--ai.${runtimeClass}${bad ? '.lnode--err' : ''}`, {
    dataset: { node: role, role },
    tabindex: '0',
    role: 'button',
    onClick: () => openRoleDrawer(role, draft, data, paint, ctx),
    onKeyDown: activateOnKey,
  });

  const dot = h('i.lnode__st');
  if (bad) dot.classList.add('bad');
  node.appendChild(dot);
  node.appendChild(h('div.lnode__id', { text: title }));
  node.appendChild(h('div.lnode__role', { text: subtitle }));

  const chips = h('div.lnode__cfg');
  if (!config) {
    chips.appendChild(h('span.chip.chip--off', { text: '未配置' }));
  } else {
    const identity = `${labels.runtimeLabel(config.runtime)} · ${config.model}`;
    chips.appendChild(h(`span.chip${bad ? '.chip--err' : ''}`, { text: identity }));
    chips.appendChild(h('span.chip', { text: config.effort || '默认' }));
    chips.appendChild(
      h(`span.chip.${config.enabled ? 'chip--on' : 'chip--off'}`, { text: config.enabled ? '启用' : '关闭' })
    );
    if (config.provenance === 'project') chips.appendChild(h('span.chip', { text: '项目覆盖' }));
  }
  node.appendChild(chips);
  return node;
}

function legend() {
  const row = h('div.legend-row.mt-4');
  const swatches = [
    ['Claude Code', '#b7c6e5', '#eef2ff', 'solid'],
    ['Codex', '#d3bff2', '#f5efff', 'solid'],
    ['实现者', '#a9e3d3', '#e8faf5', 'solid'],
    ['固定系统步骤', '#d4c4a8', '#f0e8d8', 'dashed'],
    ['人工停顿', '#f7cd67', '#fff8e0', 'solid'],
  ];
  for (const [label, border, background, style] of swatches) {
    const item = h('span');
    const sw = h('span.sw');
    sw.style.setProperty('border-color', border);
    sw.style.setProperty('background', background);
    sw.style.setProperty('border-style', style);
    item.appendChild(sw);
    item.appendChild(text(label));
    row.appendChild(item);
  }
  row.appendChild(h('span', [h('span.ln'), text('通过')]));
  row.appendChild(h('span', [h('span.ln.rej'), text('驳回 / 返工')]));
  const note = h('span.muted.ml-auto', { text: '每个角色可单独关闭 · 关闭评审时裁决一并跳过' });
  row.appendChild(note);
  return row;
}

/**
 * Draws the edges once the boxes have a measured position. Straight from the
 * design document's own routine: same four cases, same curve depths, same
 * colours — the picture is the specification.
 */
function drawEdges(host) {
  const graphEl = host.querySelector('#loop-graph');
  const layer = host.querySelector('#edge-layer');
  if (!graphEl || !layer) return;
  for (const stale of Array.from(layer.querySelectorAll('path, text'))) stale.remove();
  const bounds = graphEl.getBoundingClientRect();
  if (!bounds.width) return;

  const box = (key) => {
    const el = graphEl.querySelector(`[data-node="${key}"]`);
    if (!el) return null;
    const rect = el.getBoundingClientRect();
    return {
      x: rect.left - bounds.left,
      y: rect.top - bounds.top,
      w: rect.width,
      h: rect.height,
      lane: Number(el.closest('.lane').dataset.lane),
    };
  };

  for (const [from, to, options = {}] of EDGES) {
    const a = box(from);
    const b = box(to);
    if (!a || !b) continue;
    const sx = options.sx || 0;
    const ex = options.ex || 0;
    const t = options.t === undefined ? 0.5 : options.t;
    let d;
    let lx;
    let ly;

    if (a.lane === b.lane && !options.via) {
      const x1 = a.x + a.w;
      const y1 = a.y + a.h / 2;
      const x2 = b.x;
      const y2 = b.y + b.h / 2;
      d = `M${x1},${y1} L${x2},${y2}`;
      lx = x1 + (x2 - x1) * t;
      ly = y1 - 8;
    } else if (a.lane === b.lane) {
      const depth = options.d || 30;
      const x1 = a.x + a.w / 2 + sx;
      const x2 = b.x + b.w / 2 + ex;
      const y = options.via === 'top' ? a.y : a.y + a.h;
      const sign = options.via === 'top' ? -1 : 1;
      d = `M${x1},${y} C${x1},${y + sign * depth} ${x2},${y + sign * depth} ${x2},${y}`;
      lx = (x1 + x2) / 2;
      ly = y + sign * depth * 0.78 + (sign < 0 ? -2 : 4);
    } else if (a.lane < b.lane) {
      const x1 = a.x + a.w / 2 + sx;
      const y1 = a.y + a.h;
      const x2 = b.x + b.w / 2 + ex;
      const y2 = b.y;
      const gy = y1 + (options.gy === undefined ? 30 : options.gy);
      d = `M${x1},${y1} L${x1},${gy} L${x2},${gy} L${x2},${y2}`;
      lx = x1 + (x2 - x1) * t;
      ly = gy - 6;
    } else {
      const x1 = a.x + a.w / 2 + sx;
      const y1 = a.y;
      const x2 = b.x + b.w / 2 + ex;
      const y2 = b.y + b.h;
      const gy = y1 - (options.gy === undefined ? 30 : options.gy);
      d = `M${x1},${y1} L${x1},${gy} L${x2},${gy} L${x2},${y2}`;
      lx = x1 + (x2 - x1) * t;
      ly = gy - 6;
    }

    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', d);
    path.setAttribute('fill', 'none');
    path.setAttribute('stroke', options.rej ? '#e59266' : '#19c8b9');
    path.setAttribute('stroke-width', '2.5');
    path.setAttribute('stroke-linecap', 'round');
    path.setAttribute('stroke-linejoin', 'round');
    if (options.rej) path.setAttribute('stroke-dasharray', '6 5');
    path.setAttribute('marker-end', options.rej ? 'url(#arrRej)' : 'url(#arrPass)');
    layer.appendChild(path);

    if (options.label) {
      const label = document.createElementNS('http://www.w3.org/2000/svg', 'text');
      label.setAttribute('x', String(lx));
      label.setAttribute('y', String(ly));
      label.setAttribute('text-anchor', 'middle');
      label.setAttribute('font-size', '10.5');
      label.setAttribute('font-weight', '800');
      label.setAttribute('fill', options.rej ? '#a0522d' : '#0b6f66');
      label.setAttribute('stroke', '#f8f8f0');
      label.setAttribute('stroke-width', '4');
      label.setAttribute('paint-order', 'stroke');
      label.textContent = options.label;
      layer.appendChild(label);
    }
  }
}

// ---------------------------------------------------------------------------
// The role drawer — C-04's five fields
// ---------------------------------------------------------------------------

function openRoleDrawer(role, draft, data, paint, ctx) {
  const config = draft[role];
  if (!config) return;
  const skills = data.skills || [];
  const projectId = data.projectId;

  const form = h('div.form');
  const apply = () => {
    close();
    paint();
  };

  // 启用
  const toggle = h('span.switch.switch--sm', {
    role: 'switch',
    tabindex: '0',
    'aria-checked': config.enabled ? 'true' : 'false',
  });
  if (config.enabled) toggle.classList.add('checked');
  const flip = () => {
    config.enabled = !config.enabled;
    toggle.classList.toggle('checked', config.enabled);
    toggle.setAttribute('aria-checked', config.enabled ? 'true' : 'false');
    refreshImpact();
  };
  toggle.addEventListener('click', flip);
  toggle.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      flip();
    }
  });
  form.appendChild(h('label', { text: '启用' }));
  form.appendChild(
    h('span.row', [toggle, h('span.small.muted', { text: '关闭后该节点跳过' })])
  );

  // CLI
  const runtimeSelect = h('select.select', {
    onChange: (event) => {
      config.runtime = event.target.value;
      refreshImpact();
    },
  });
  for (const [value, label] of Object.entries(labels.RUNTIME_LABELS)) {
    const option = h('option', { value, text: label });
    if (config.runtime === value) option.selected = true;
    runtimeSelect.appendChild(option);
  }
  form.appendChild(h('label', { text: 'CLI' }));
  form.appendChild(runtimeSelect);

  // 模型 — free text, because the core does not publish a model catalogue and
  // inventing one here would go stale the week a CLI ships a new name.
  const modelInput = h('input', {
    type: 'text',
    value: config.model || '',
    placeholder: 'claude-opus-5',
    onInput: (event) => {
      config.model = event.target.value.trim();
      refreshImpact();
    },
  });
  form.appendChild(h('label', { text: '模型' }));
  form.appendChild(h('label.input', [modelInput]));

  // Effort — empty means "use the CLI default", which is a real value.
  const effortSelect = h('select.select', {
    onChange: (event) => {
      config.effort = event.target.value || null;
    },
  });
  for (const [value, label] of EFFORT_OPTIONS) {
    const option = h('option', { value, text: label });
    if ((config.effort || '') === value) option.selected = true;
    effortSelect.appendChild(option);
  }
  form.appendChild(h('label', { text: 'Effort' }));
  form.appendChild(effortSelect);

  // 绑定的技能 — S-03: bound means the role must use it.
  const skillRow = h('div.row');
  if (!skills.length) {
    skillRow.appendChild(h('span.small.muted', { text: '没有盘点到技能' }));
  }
  for (const skill of skills) {
    const pill = tag(skill.name, 'outlined');
    const bound = config.skills.includes(skill.name);
    if (bound) pill.classList.add('on');
    const visible = (skill.visible_to || []).includes(config.runtime);
    if (!visible) pill.classList.add('tag--soft-red');
    pill.setAttribute('role', 'button');
    pill.setAttribute('tabindex', '0');
    const flipSkill = () => {
      const index = config.skills.indexOf(skill.name);
      if (index >= 0) config.skills.splice(index, 1);
      else config.skills.push(skill.name);
      pill.classList.toggle('on');
      refreshImpact();
    };
    pill.addEventListener('click', flipSkill);
    pill.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        flipSkill();
      }
    });
    skillRow.appendChild(pill);
  }
  form.appendChild(h('label', { text: '绑定的技能' }));
  form.appendChild(skillRow);

  // 配置来源
  const sourceRow = h('span.row', [
    h('span.small.muted', {
      text: projectId
        ? config.provenance === 'project'
          ? '项目覆盖 · 可恢复为全局默认'
          : '跟随全局默认'
        : '全局默认 · 项目视图中可覆盖',
    }),
  ]);
  if (projectId && config.provenance === 'project') {
    sourceRow.appendChild(
      registerWrite(
        h('button.btn.btn--sm.btn--text', {
          type: 'button',
          onClick: () =>
            attempt({
              label: `${labels.roleLabel(role)}已恢复默认`,
              success: '这个角色重新跟随全局默认。',
              run: (write) => write.resetRole(projectId, role),
              onDone: () => {
                close();
                return ctx.refresh();
              },
            }),
        }, [text('恢复默认')])
      )
    );
  }
  form.appendChild(h('label', { text: '配置来源' }));
  form.appendChild(sourceRow);

  const impact = h('div.impact.impact--error.mt-12');
  const impactText = h('div');
  impact.appendChild(icon('warn'));
  impact.appendChild(impactText);

  function refreshImpact() {
    const problems = [
      ...sameModelViolations(draft).filter((v) => labels.violationRoles(v).includes(role)),
      ...skillVisibilityProblems(role, config, skills),
    ];
    impactText.replaceChildren();
    if (!problems.length) {
      impact.classList.add('hide');
      return;
    }
    impact.classList.remove('hide');
    impactText.appendChild(h('b', { text: labels.violationText(problems[0]) }));
    impactText.appendChild(
      text(' 保存被阻止。评测侧必须与生成侧不同模型，绑定的技能必须对该角色的 CLI 可见。')
    );
  }
  refreshImpact();

  openDrawer({
    title: labels.roleLabel(role),
    body: [
      h('div.row.mb-12', [
        tag(role, 'soft-brown'),
        h(`span.srcchip.srcchip--${config.provenance === 'project' ? 'p' : 'g'}`, {
          text: config.provenance === 'project' ? '项目' : '全局',
        }),
      ]),
      form,
      impact,
      h('div.quiet.mt-12', {
        text: '保存后对下一个启动的节点生效，正在跑的会话不打断。绑定的技能 = 该角色启动时必须使用；绑定到看不见它的 CLI 会被阻止。',
      }),
    ],
    footer: [closeButton('取消'), h('button.btn.btn--yellow.ml-auto', { type: 'button', onClick: apply }, [text('应用到草稿')])],
    onClose: () => {},
  });
}

// ---------------------------------------------------------------------------
// Validation and saving
// ---------------------------------------------------------------------------

/** The same rule as `config::validate`: evaluator identity != generator's. */
export function sameModelViolations(draft) {
  const pairs = [['review', 'plan'], ['audit', 'impl']];
  const out = [];
  for (const [evaluator, generator] of pairs) {
    const e = draft[evaluator];
    const g = draft[generator];
    if (!e || !g || !e.enabled || !g.enabled) continue;
    if (`${e.runtime}:${e.model}` === `${g.runtime}:${g.model}`) {
      out.push({
        kind: 'same_model',
        evaluator,
        generator,
        identity: `${labels.runtimeLabel(e.runtime)} · ${e.model}`,
      });
    }
  }
  return out;
}

/** S-04, checked against the inventory the screen already read. */
function skillVisibilityProblems(role, config, skills) {
  const out = [];
  for (const name of config.skills) {
    const skill = skills.find((s) => s.name === name);
    if (!skill) {
      out.push({ kind: 'skill_not_found', role, skill: name });
    } else if (!(skill.visible_to || []).includes(config.runtime)) {
      out.push({ kind: 'skill_not_visible', role, skill: name, runtime: config.runtime });
    }
  }
  return out;
}

function changedRoles(data, draft) {
  const out = [];
  for (const resolved of ((data.resolved || {}).roles) || []) {
    const before = resolved.config;
    const after = draft[resolved.role];
    if (!after) continue;
    const same =
      before.enabled === after.enabled &&
      before.runtime === after.runtime &&
      before.model === after.model &&
      (before.effort || null) === (after.effort || null) &&
      (before.skills || []).join(' ') === after.skills.join(' ');
    if (!same) out.push(resolved.role);
  }
  return out;
}

/**
 * Saves the changed roles one at a time. Sequential rather than parallel: the
 * core validates each write against the resolved configuration as it stands,
 * and two colliding writes racing each other would make which one is refused
 * depend on scheduling.
 */
async function saveDraft(data, draft, ctx, projectId) {
  const dirty = changedRoles(data, draft);
  for (const role of dirty) {
    const config = draft[role];
    const result = await attempt({
      label: `已保存 ${labels.roleLabel(role)}`,
      success: '对下一个启动的节点生效。',
      run: (write) =>
        write.setRole({
          projectId: projectId || undefined,
          role,
          enabled: config.enabled,
          runtime: config.runtime,
          model: config.model,
          effort: config.effort,
          skills: config.skills,
        }),
    });
    // One refusal stops the batch: the remaining writes were computed against
    // a configuration the core just rejected.
    if (!result.ok) break;
  }
  await ctx.refresh();
}
