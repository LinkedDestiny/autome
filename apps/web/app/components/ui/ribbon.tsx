import type { HTMLAttributes } from 'react';
export function Ribbon({ color = 'teal', small = false, children, className = '', ...props }: HTMLAttributes<HTMLDivElement> & { color?: 'teal' | 'yellow' | 'blue' | 'green' | 'brown'; small?: boolean }) {
  return <div {...props} className={['ribbon', 'ribbon-' + color, small && 'ribbon-sm', className].filter(Boolean).join(' ')}><span>{children}</span></div>;
}
