export function Segmented({ name, label, options, value, onChange, disabled = false }: {
  name: string; label: string; options: readonly { value: string; label: string; disabled?: boolean }[];
  value: string; onChange: (value: string) => void; disabled?: boolean;
}) {
  return <div className="seg" role="radiogroup" aria-label={label}>
    {options.map(option => <label key={option.value}>
      <input type="radio" name={name} value={option.value} checked={value === option.value} disabled={disabled || option.disabled} onChange={() => onChange(option.value)} />
      <span>{option.label}</span>
    </label>)}
  </div>;
}

/** URL-backed tabs retain native links, history and modified-click navigation. */
export function SegmentedTabs({ label, className = '', children }: { label: string; className?: string; children: import('react').ReactNode }) {
  return <nav className={`seg ${className}`} aria-label={label}>{children}</nav>;
}
