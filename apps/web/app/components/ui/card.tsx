import type { HTMLAttributes } from 'react';
export type CardColor = 'yellow' | 'teal' | 'purple' | 'blue' | 'green' | 'red';
export function Card({ color, patterned = false, dashed = false, className = '', ...props }: HTMLAttributes<HTMLElement> & { color?: CardColor; patterned?: boolean; dashed?: boolean }) {
  return <section {...props} className={['card', color && 'card-' + color, patterned && 'card-pat', dashed && 'card-dash', className].filter(Boolean).join(' ')} />;
}
