import { TEST_IDS } from '../lib/testids.ts';
import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router';
import { filterCandidates, paletteReducer, type Candidate, type PaletteAction, type PaletteState } from './palette-model.ts';

export function CommandPalette({ candidates, state, onChange }: { candidates: Candidate[]; state: PaletteState; onChange: (state: PaletteState) => void }) {
  const input = useRef<HTMLInputElement>(null);
  const navigate = useNavigate();
  const { items, remaining } = filterCandidates(candidates, state.query);
  const optionId = (index: number) => `palette-option-${index}`;
  useEffect(() => {
    if (!state.open) return;
    const previous = document.activeElement;
    input.current?.focus();
    return () => { if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, [state.open]);
  useEffect(() => {
    if (state.open && state.active >= 0) document.getElementById(`palette-option-${state.active}`)?.scrollIntoView({ block: 'nearest' });
  }, [state.open, state.active]);
  function dispatch(action: PaletteAction) {
    const next = paletteReducer(state, action);
    onChange(next);
    if (next.navigate) navigate(next.navigate);
  }
  if (!state.open) return null;
  return <div className="palette-backdrop" onClick={event => { if (event.target === event.currentTarget) dispatch({ type: 'close' }); }}>
    <section className="palette" data-testid={TEST_IDS.palette} role="dialog" aria-modal="true" aria-label="跳转" onKeyDown={event => {
      if (event.key === 'Escape') { event.preventDefault(); dispatch({ type: 'close' }); }
      if (event.key === 'Tab') {
        const controls = event.currentTarget.querySelectorAll<HTMLElement>('button, input');
        const first = controls[0], last = controls[controls.length - 1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
        if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
      }
    }}>
      <div className="palette-header"><label htmlFor="palette-query">搜索或跳转</label><button className="btn btn-sm" type="button" onClick={() => dispatch({ type: 'close' })}>关闭</button></div>
      <input ref={input} id="palette-query" role="combobox" aria-expanded="true" aria-controls="palette-results" aria-autocomplete="list"
        aria-activedescendant={items[state.active] ? optionId(state.active) : undefined} value={state.query} placeholder="输入任务、项目或新建"
        onChange={event => { const query = event.target.value; dispatch({ type: 'input', query, items: filterCandidates(candidates, query).items }); }}
        onKeyDown={event => {
          if (event.nativeEvent.isComposing) return;
          const keys: Record<string, 'down' | 'up' | 'enter' | 'close'> = { ArrowDown: 'down', ArrowUp: 'up', Enter: 'enter', Escape: 'close' };
          const type = keys[event.key];
          if (type) { event.preventDefault(); dispatch(type === 'close' ? { type } : { type, items }); }
        }} />
      <div className="palette-list" data-scroll-region id="palette-results" role="listbox" aria-label="跳转结果">
        {items.map((item, index) => <div key={item.key} id={optionId(index)} role="option" aria-selected={index === state.active} aria-disabled={item.disabled} title={item.reason}
          className="palette-option" onClick={() => { if (!item.disabled) { onChange({ ...state, open: false }); navigate(item.href); } }}>
          <strong>{item.title}</strong><span>{item.id} · {item.subtitle}</span>
        </div>)}
        {items.length === 0 && <p>没有匹配的结果</p>}
      </div>
      {remaining > 0 && <p className="palette-hint">还有 {remaining} 条，继续输入可缩小范围</p>}
    </section>
  </div>;
}
