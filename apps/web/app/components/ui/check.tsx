import { useId, type InputHTMLAttributes, type ReactNode } from 'react';
type CheckProps = Omit<InputHTMLAttributes<HTMLInputElement>, 'type' | 'disabled'> & { label: ReactNode } & ({ disabled: true; reason: string } | { disabled?: false; reason?: string });
export function Check({ label, reason, disabled, className = '', 'aria-describedby': describedBy, ...props }: CheckProps) {
  const reasonId = useId();
  return <div><label className={['chk', className].filter(Boolean).join(' ')}>
    <input {...props} type="checkbox" disabled={disabled} aria-describedby={[describedBy, disabled && reasonId].filter(Boolean).join(' ') || undefined} />{label}
  {disabled && <small id={reasonId} className="chk-reason" title={reason}>{reason}</small>}</label></div>;
}
