import type { InputHTMLAttributes, ReactNode } from 'react';
export function Switch({ label, className = '', ...props }: Omit<InputHTMLAttributes<HTMLInputElement>, 'type' | 'role'> & { label: ReactNode }) {
  return <label className={['switch', className].filter(Boolean).join(' ')}><input {...props} type="checkbox" role="switch" /><span aria-hidden="true" /><span>{label}</span></label>;
}
