import { useEffect, useRef, type ReactNode } from 'react';
import { manageDrawerFocus } from './drawer-focus.ts';
export function Drawer({ label, title, onClose, footer, className = '', children }: {
  label: string; title: ReactNode; onClose: () => void; footer: ReactNode; className?: string; children: ReactNode;
}) {
  const container = useRef<HTMLElement>(null);
  useEffect(() => {
    if (container.current) return manageDrawerFocus(container.current);
  }, []);
  return <aside ref={container} tabIndex={-1} className={`drawer ${className}`} role="dialog" aria-modal="false" aria-label={label} onKeyDown={e => { if (e.key === 'Escape') onClose(); }}>
    <header className="drawer-head"><h2>{title}</h2><button type="button" className="btn btn-sm drawer-close" onClick={onClose}>关闭</button></header>
    <div className="drawer-body" data-scroll-region>{children}</div>
    <footer className="drawer-foot">{footer}</footer>
  </aside>;
}
