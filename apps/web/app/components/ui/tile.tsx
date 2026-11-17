import type { HTMLAttributes, ReactNode } from 'react';
export function Tile({ color, value, label, children, className = '', ...props }: HTMLAttributes<HTMLDivElement> & { color: 'teal' | 'yellow' | 'blue' | 'pink' | 'purple' | 'orange' | 'green'; value?: string | number | null; label: ReactNode }) {
  const missing = value == null;
  return <div {...props} className={['tile', 'tile-' + color, className].filter(Boolean).join(' ')} data-missing={missing || undefined}>{label}<b>{missing ? '—' : value}</b>{children && <small>{children}</small>}</div>;
}
