export function Progress({ value, label }: { value: number; label: string }) {
  const pct = Number.isNaN(value) ? 0 : Math.max(0, Math.min(100, value));
  return <div className="prog" role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
    <svg viewBox="0 0 100 10" preserveAspectRatio="none" aria-hidden="true"><rect width={pct} height="10" rx="5" /></svg>
  </div>;
}
