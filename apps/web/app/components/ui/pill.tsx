import type { HTMLAttributes } from 'react';
export type PillColor = 'teal' | 'blue' | 'purple' | 'orange' | 'pink' | 'brown' | 'green' | 'yellow' | 'red';
export function Pill({ color = 'teal', filled = false, className = '', ...props }: HTMLAttributes<HTMLSpanElement> & { color?: PillColor; filled?: boolean }) {
  return <span {...props} className={['pill', 'pill-' + color, filled && 'pill-fill', className].filter(Boolean).join(' ')} />;
}
