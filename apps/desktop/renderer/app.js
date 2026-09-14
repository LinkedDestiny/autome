'use strict';

// Thin DOM glue over the pure modules (nav.js/axis.js/status.js/
// exec-bar.js). Not unit-tested itself (there is no DOM in node:test);
// verified by launching the real Electron window (`npm start`) and by
// test/dom.test.js driving the real DOM through test/dom-harness.js.
// Every judgment call (what tone a state gets, what a panel contains)
// happens in the pure modules — this file only decides which DOM node a
// value lands in.
(function () {
  const nav = globalThis.AutomeNav;
  const status = globalThis.AutomeStatus;
  const execBar = globalThis.AutomeExecBar;

  let state = nav.initialState();

  function el(tag, props, children) {
    const node = document.createElement(tag);
    if (props) {
      for (const [key, value] of Object.entries(props)) {
        if (key === 'className') node.className = value;
        else if (key === 'text') node.textContent = value;
        else if (key.startsWith('on') && typeof value === 'function') {
          node.addEventListener(key.slice(2).toLowerCase(), value);
        } else if (key === 'dataset') {
          Object.assign(node.dataset, value);
        } else if (value === true) {
          // Boolean DOM properties (disabled, checked, ...) are governed
          // by attribute *presence*, not its string value -- setAttribute
          // ('disabled', false) still renders disabled="false", which the
          // DOM treats as disabled. `true` -> present; `false`/`null`/
          // `undefined` -> omitted entirely. Existing string-valued call
          // sites (`disabled: 'true'`) are untouched by this branch.
          node.setAttribute(key, '');
        } else if (value === false || value === undefined || value === null) {
          // omit -- see boolean-prop note above.
        } else {
          node.setAttribute(key, value);
        }
      }
    }
    for (const child of children || []) {
      if (child) node.appendChild(child);
    }
    return node;
  }

  function chipNode(chip) {
    return el('span', { className: `ac-chip ac-chip--${chip.tone}${chip.dashed ? ' ac-chip--dashed' : ''}` }, [
      el('span', { className: 'ac-chip__icon', text: chip.icon }),
      el('span', { className: 'ac-chip__label', text: chip.label }),
    ]);
  }

  function axisStripNode(cells) {
    return el(
      'div',
      { className: 'ac-axis-strip' },
      cells.map((cell) =>
        el('div', { className: 'ac-axis-cell' }, [
          el('div', { className: 'ac-axis-cell__label', text: cell.axisZh }),
          chipNode(status.statusChip(cell.observed ? 'ok' : 'unobserved', cell.valueZh)),
        ])
      )
    );
  }

  function defaultExecBarModel() {
    return execBar.buildExecBar({ connection: { status: 'disconnected' } });
  }

  function renderExecBar(mountEl, bar) {
    mountEl.innerHTML = '';
    const lanes = [
      ['当前运行', bar.running],
      ['等待我', bar.waitingOnMe],
      [`排队 ${bar.queuedCount}`, bar.queued],
    ];
    mountEl.appendChild(
      el('div', { className: 'ac-exec-bar' }, [
        el('div', { className: 'ac-exec-bar__status' }, [
          chipNode(status.statusChip('unobserved', bar.asOfLabel || '截至未知时间')),
          bar.writeDisabled ? el('span', { className: 'ac-exec-bar__write-disabled', text: '写操作已禁用' }) : null,
        ]),
        el(
          'div',
          { className: 'ac-exec-bar__lanes' },
          lanes.map(([label, items]) =>
            el('div', { className: 'ac-exec-bar__lane' }, [
              el('span', { className: 'ac-exec-bar__lane-label', text: label }),
              items.length
                ? el(
                    'ul',
                    { className: 'ac-exec-bar__lane-list' },
                    items.map((item) => el('li', { text: `${item.projectLabel} · ${item.taskId}` }))
                  )
                : el('span', { className: 'ac-exec-bar__lane-empty', text: '（无）' }),
            ])
          )
        ),
      ])
    );
  }

  function renderNav(navEl) {
    navEl.innerHTML = '';
    for (const tab of nav.TABS) {
      navEl.appendChild(
        el('button', {
          type: 'button',
          className: `ac-tab${tab.id === state.activeTabId ? ' ac-tab--active' : ''}`,
          text: tab.label,
          dataset: { tabId: tab.id },
          'aria-pressed': String(tab.id === state.activeTabId),
          onclick: () => {
            state = nav.selectTab(state, tab.id);
            render();
          },
        })
      );
    }
  }

  function noProjectPanel(panel) {
    const disabled = panel.connected !== true;
    const disabledReason = disabled ? 'Core 不可达，写操作已禁用' : null;
    return el('div', { className: 'ac-card ac-fade-up' }, [
      el('p', { className: 'ac-empty-heading', text: panel.heading }),
      el(
        'div',
        { className: 'ac-cta-row' },
        panel.ctas.map((label, index) =>
          el('button', {
            type: 'button',
            className: 'ac-btn ac-btn--primary',
            text: label,
            disabled,
            title: disabledReason || undefined,
            onclick: () => {
              state = nav.beginProjectDraft(state, panel.ctaKinds[index]);
              render();
            },
          })
        )
      ),
      disabledReason ? el('p', { className: 'ac-cta-row__reason', text: disabledReason }) : null,
      el('section', { className: 'ac-reference ac-fade-up' }, [
        el('h2', { className: 'ac-reference__title', text: '预览：一个 Task 要走哪 17 步' }),
        el(
          'ol',
          { className: 'ac-phase-rail' },
          panel.taskPhaseRail.map((step) =>
            el('li', { className: 'ac-phase-rail__step' }, [chipNode(step.chip), el('span', { text: step.zh })])
          )
        ),
      ]),
      el('section', { className: 'ac-reference ac-fade-up' }, [
        el('h2', { className: 'ac-reference__title', text: `"完成"的定义：${panel.completionGates.length} 项检查` }),
        el(
          'ul',
          { className: 'ac-gate-list' },
          panel.completionGates.map((gate) => el('li', { className: 'ac-gate-list__item' }, [chipNode(gate.chip), el('span', { text: gate.zh })]))
        ),
      ]),
    ]);
  }

  function emptyStatePanel(panel) {
    const children = [
      el('h2', { className: 'ac-empty-heading', text: panel.title }),
      el('p', { className: 'ac-empty-reason', text: panel.reason }),
    ];
    if (panel.axesVocabulary) {
      children.push(
        el('section', { className: 'ac-reference' }, [
          el('h3', { className: 'ac-reference__title', text: '环境组件的五条正交事实轴' }),
          el(
            'div',
            { className: 'ac-axis-vocab' },
            panel.axesVocabulary.map((axis) =>
              el('div', { className: 'ac-axis-vocab__axis' }, [
                el('strong', { text: axis.zh }),
                el('span', { text: axis.values.map((v) => v.zh).join(' / ') }),
              ])
            )
          ),
        ])
      );
    }
    if (panel.evidenceLadder) {
      children.push(
        el('section', { className: 'ac-reference' }, [
          el('h3', { className: 'ac-reference__title', text: 'Skill 六级证据阶梯' }),
          el(
            'ol',
            { className: 'ac-ladder' },
            panel.evidenceLadder.map((rung) => el('li', {}, [chipNode(status.statusChip('unobserved', rung.zh))]))
          ),
        ])
      );
    }
    return el('div', { className: 'ac-card ac-fade-up' }, children);
  }

  function projectListPanel(panel) {
    return el('div', { className: 'ac-card ac-fade-up' }, [
      el('h2', { className: 'ac-empty-heading', text: `Project（${panel.items.length}）` }),
      el(
        'ul',
        { className: 'ac-project-list' },
        panel.items.map((item) =>
          el('li', { className: 'ac-project-list__item' }, [
            el('button', {
              type: 'button',
              className: 'ac-project-list__select',
              text: item.displayName,
              onclick: () => {
                state = nav.selectProject(state, item.id);
                render();
              },
            }),
            axisStripNode(item.axes),
          ])
        )
      ),
    ]);
  }

  function projectSubtabContentNode(subtabPanel) {
    switch (subtabPanel.subtabId) {
      case 'overview':
        return el('section', { className: 'ac-reference' }, [
          el('h3', { className: 'ac-reference__title', text: subtabPanel.title }),
          axisStripNode(subtabPanel.axes),
        ]);
      case 'tasks':
        return el('section', { className: 'ac-reference' }, [
          el('h3', { className: 'ac-reference__title', text: subtabPanel.title }),
          axisStripNode(subtabPanel.runAxes),
          el(
            'ol',
            { className: 'ac-phase-rail' },
            subtabPanel.linearPhaseRail.map((step) =>
              el('li', { className: 'ac-phase-rail__step' }, [chipNode(step.chip), el('span', { text: step.zh })])
            )
          ),
        ]);
      case 'config':
        return el('section', { className: 'ac-reference' }, [
          el('h3', { className: 'ac-reference__title', text: subtabPanel.title }),
          el('table', { className: 'ac-config-matrix' }, [
            el('thead', {}, [
              el(
                'tr',
                {},
                subtabPanel.matrixColumns.map((col) => el('th', { text: col }))
              ),
            ]),
            el(
              'tbody',
              {},
              subtabPanel.fixedRows.map((row) =>
                el(
                  'tr',
                  { className: row.overridable ? '' : 'ac-config-matrix__locked' },
                  [el('td', { text: row.zh })]
                )
              )
            ),
          ]),
        ]);
      case 'environment-skills':
        return emptyStatePanel(subtabPanel);
      case 'history-evidence':
        return el('section', { className: 'ac-reference' }, [
          el('h3', {
            className: 'ac-reference__title',
            text: `"完成"的定义：${subtabPanel.completionGates.length} 项检查`,
          }),
          el(
            'ul',
            { className: 'ac-gate-list' },
            subtabPanel.completionGates.map((gate) =>
              el('li', { className: 'ac-gate-list__item' }, [chipNode(gate.chip), el('span', { text: gate.zh })])
            )
          ),
          el(
            'div',
            { className: 'ac-evidence-columns' },
            subtabPanel.evidenceTableColumns.map((col) =>
              el('span', { className: 'ac-evidence-columns__col', text: col })
            )
          ),
        ]);
      default:
        throw new Error(`unknown project subtab id: ${subtabPanel.subtabId}`);
    }
  }

  function projectDetailPanel(panel) {
    return el('div', { className: 'ac-card ac-fade-up' }, [
      el('div', { className: 'ac-project-detail__header' }, [
        el('button', {
          type: 'button',
          className: 'ac-btn ac-btn--secondary',
          text: '← 返回项目列表',
          onclick: () => {
            state = nav.deselectProject(state);
            render();
          },
        }),
        el('h2', { className: 'ac-project-detail__name', text: panel.project.displayName }),
        axisStripNode(panel.project.axes),
      ]),
      el(
        'nav',
        { className: 'ac-project-subtabs' },
        nav.PROJECT_SUBTABS.map((tab) =>
          el('button', {
            type: 'button',
            className: `ac-tab${tab.id === panel.subtabId ? ' ac-tab--active' : ''}`,
            text: tab.label,
            onclick: () => {
              state = nav.selectProjectSubtab(state, tab.id);
              render();
            },
          })
        )
      ),
      projectSubtabContentNode(panel.subtabPanel),
    ]);
  }

  // §8.2 project draft: kind -> pick a directory -> target summary ->
  // name -> (new_product) destination name / (existing_repository) trust
  // checkbox -> submit. `panel` is nav.js's pure `projectDraftPanel(state)`
  // output -- this function only decides which DOM node each field lands
  // in, exactly like every other *Panel function in this file.
  function projectDraftFieldNode(panel, field, label, placeholder) {
    return el('label', { className: 'ac-draft-field' }, [
      el('span', { className: 'ac-draft-field__label', text: label }),
      el('input', {
        type: 'text',
        className: 'ac-draft-field__input',
        'data-draft-field': field,
        value: panel[field],
        placeholder,
        disabled: panel.submitting,
        oninput: (event) => {
          state = nav.setProjectDraftField(state, field, event.target.value);
          render();
        },
      }),
    ]);
  }

  function projectDraftSummaryNode(panel) {
    if (!panel.targetSummaryCells) return null;
    return el(
      'div',
      { className: 'ac-draft-summary' },
      panel.targetSummaryCells.map((cell) =>
        el('div', { className: 'ac-draft-summary__cell' }, [
          el('span', { className: 'ac-draft-summary__label', text: cell.zh }),
          cell.shape === 'chip'
            ? chipNode(cell.chip)
            : el('span', { className: 'ac-draft-summary__value', text: cell.text }),
        ])
      )
    );
  }

  function projectDraftPanelNode(panel) {
    const pickTarget = async () => {
      try {
        const result = await globalThis.automeWrite.pickProjectTarget(panel.draftKind);
        if (result && result.cancelled) return;
        state = nav.setProjectDraftTarget(state, result);
      } catch (err) {
        state = nav.setProjectDraftError(state, String((err && err.message) || err));
      }
      render();
    };

    const submitDraft = async () => {
      if (!panel.submittable) return;
      state = nav.setProjectDraftSubmitting(state, true);
      render();
      try {
        await globalThis.automeWrite.createProject({
          targetId: panel.target.target_id,
          displayName: panel.displayName,
          trustConfirmed: panel.trustConfirmed,
          destinationName: panel.draftKind === 'new_product' ? panel.destinationName : null,
        });
        state = nav.cancelProjectDraft(state);
        render();
        refreshExecBarFromCore();
      } catch (err) {
        state = nav.setProjectDraftError(state, String((err && err.message) || err));
        render();
      }
    };

    const cancelDraft = () => {
      state = nav.cancelProjectDraft(state);
      render();
    };

    const kindField =
      panel.draftKind === 'new_product'
        ? projectDraftFieldNode(panel, 'destinationName', '新目录名称', '例如 my-new-product')
        : el('label', { className: 'ac-draft-field ac-draft-field--checkbox' }, [
            el('input', {
              type: 'checkbox',
              'data-draft-field': 'trustConfirmed',
              checked: panel.trustConfirmed,
              disabled: panel.submitting,
              onchange: (event) => {
                state = nav.setProjectDraftField(state, 'trustConfirmed', event.target.checked);
                render();
              },
            }),
            el('span', { text: '信任此仓库（≠ 打开 CLI 的 project trust）' }),
          ]);

    return el('div', { className: 'ac-card ac-fade-up ac-draft' }, [
      el('div', { className: 'ac-draft__header' }, [
        el('button', {
          type: 'button',
          className: 'ac-btn ac-btn--secondary',
          text: '← 取消',
          onclick: cancelDraft,
        }),
        el('h2', { className: 'ac-draft__heading', text: panel.heading }),
      ]),
      el('div', { className: 'ac-draft__section' }, [
        el('button', {
          type: 'button',
          className: 'ac-btn ac-btn--secondary',
          text: panel.pickLabel,
          disabled: panel.submitting,
          onclick: pickTarget,
        }),
        projectDraftSummaryNode(panel),
      ]),
      el('div', { className: 'ac-draft__section' }, [
        projectDraftFieldNode(panel, 'displayName', '项目名称', '给这个项目起个名字'),
        kindField,
      ]),
      panel.error ? el('p', { className: 'ac-draft-error', text: panel.error }) : null,
      panel.blockers.length > 0
        ? el(
            'ul',
            { className: 'ac-draft-blockers' },
            panel.blockers.map((blocker) => el('li', { className: 'ac-draft-blockers__item', text: blocker }))
          )
        : null,
      el('button', {
        type: 'button',
        className: 'ac-btn ac-btn--primary',
        text: panel.submitting ? '创建中…' : '创建项目',
        disabled: !panel.submittable,
        onclick: submitDraft,
      }),
    ]);
  }

  function renderPanel(panelEl) {
    // Rebuilding the panel subtree on every keystroke (draft text fields
    // re-render through the same full render() path as everything else --
    // see the field's `oninput` above) would otherwise steal focus and
    // reset the caret after every character. Save/restore by the input's
    // own `data-draft-field` marker, the one thing stable across a
    // teardown+rebuild of an otherwise brand-new node.
    const active = document.activeElement;
    const activeField = active instanceof Element ? active.getAttribute('data-draft-field') : null;
    const selectionStart = active && typeof active.selectionStart === 'number' ? active.selectionStart : null;
    const selectionEnd = active && typeof active.selectionEnd === 'number' ? active.selectionEnd : null;

    panelEl.innerHTML = '';
    const panel = nav.panelFor(state);
    if (panel.kind === 'no-project-empty-state') {
      panelEl.appendChild(noProjectPanel(panel));
    } else if (panel.kind === 'project-list') {
      panelEl.appendChild(projectListPanel(panel));
    } else if (panel.kind === 'project-detail') {
      panelEl.appendChild(projectDetailPanel(panel));
    } else if (panel.kind === 'project-draft') {
      panelEl.appendChild(projectDraftPanelNode(panel));
    } else {
      panelEl.appendChild(emptyStatePanel(panel));
    }

    if (activeField) {
      const toFocus = panelEl.querySelector(`[data-draft-field="${activeField}"]`);
      if (toFocus) {
        toFocus.focus();
        if (selectionStart !== null && typeof toFocus.setSelectionRange === 'function') {
          toFocus.setSelectionRange(selectionStart, selectionEnd);
        }
      }
    }
  }

  // Cache of the last exec-bar model actually observed from Core. render()
  // must read this rather than always reverting to defaultExecBarModel(),
  // otherwise any unrelated re-render (a nav tab click, a draft keystroke)
  // would silently regress a connected exec bar back to "disconnected".
  let lastExecBarModel = null;

  function render() {
    renderExecBar(document.getElementById('exec-bar'), lastExecBarModel || defaultExecBarModel());
    renderNav(document.getElementById('top-nav'));
    renderPanel(document.getElementById('panel'));
  }

  // Best-effort async refresh, run once after the first fully-unobserved
  // paint (and again after a successful draft submission). `automeRead` is
  // absent under the DOM harness (no Main process) and its calls reject
  // whenever no `ipcMain.handle('autome:read', ...)` is listening (also
  // true under the harness) or the sidecar is down — both cases fall back
  // to the default render() already on screen, silently, since neither is
  // a renderer-visible error condition. §9.3: only a successful read ever
  // flips `connected` true — it stays false, and every write affordance
  // stays disabled, until Core actually answers something.
  function refreshExecBarFromCore() {
    const automeRead = globalThis.automeRead;
    if (!automeRead) return;
    if (typeof automeRead.getQueue === 'function') {
      automeRead
        .getQueue()
        .then((queuePayload) => {
          lastExecBarModel = execBar.fromQueueSnapshot(
            queuePayload.state,
            { status: 'connected' },
            queuePayload.entry_labels
          );
          state = nav.setConnectionStatus(state, true);
          render();
        })
        .catch(() => {});
    }
    if (typeof automeRead.listProjects === 'function') {
      automeRead
        .listProjects()
        .then((payload) => {
          state = nav.setProjects(state, (payload && payload.projects) || []);
          state = nav.setConnectionStatus(state, true);
          render();
        })
        .catch(() => {});
    }
  }

  render();
  refreshExecBarFromCore();
})();
