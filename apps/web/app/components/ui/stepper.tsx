import { useId } from 'react';
export function Stepper({ value, min, max, onChange, label, name, disabled = false }: {
  value: number; min: number; max: number; onChange: (value: number) => void; label: string; name?: string; disabled?: boolean;
}) {
  const id = useId();
  const change = (next: number) => { if (Number.isFinite(next)) onChange(Math.max(min, Math.min(max, Math.round(next)))); };
  return <div><label htmlFor={id}>{label}</label><div className="stepper">
    <button type="button" aria-label={'减少' + label} disabled={disabled || value <= min} onClick={() => change(value - 1)}>−</button>
    <input id={id} name={name} type="number" min={min} max={max} step={1} value={value} disabled={disabled} onChange={event => change(event.currentTarget.valueAsNumber)} />
    <button type="button" aria-label={'增加' + label} disabled={disabled || value >= max} onClick={() => change(value + 1)}>＋</button>
  </div></div>;
}
