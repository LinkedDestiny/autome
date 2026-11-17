import type { ButtonHTMLAttributes } from 'react';
export function Button({ variant = 'default', small = false, className = '', type = 'button', ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: 'default' | 'primary' | 'text'; small?: boolean }) {
  return <button {...props} type={type} className={['btn', variant !== 'default' && 'btn-' + variant, small && 'btn-sm', className].filter(Boolean).join(' ')} />;
}
