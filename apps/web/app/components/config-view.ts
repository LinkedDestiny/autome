import { roleLabel } from '../lib/labels.ts';
import { CONFIG_LABELS, SETTINGS } from '../lib/admin-labels.ts';
export type ConfigRow = { key: string; label: string; value: string; isDefault?: boolean; source?: string };
export function configRows(resolved: unknown, defaults: readonly string[] = []): ConfigRow[] {
  const rows: ConfigRow[] = [];
  function visit(value: unknown, key: string) {
    if (Array.isArray(value) && key === 'roles') {
      value.forEach((role: Record<string, unknown>, i) => rows.push({ key: `${key}.${i}`, label: `${SETTINGS.role} ${roleLabel(String(role.role ?? role.name ?? i))}（${role.role ?? role.name ?? i}）`,
        value: Object.entries((role.config ?? role) as Record<string, unknown>).filter(([name]) => name !== 'role' && name !== 'name').map(([name, v]) => name === 'enabled' ? v ? SETTINGS.enabled : SETTINGS.disabled : v === null ? SETTINGS.unset : typeof v === 'object' ? JSON.stringify(v) : String(v)).join(' · ') }));
    } else if (value !== null && typeof value === 'object') {
      Object.entries(value).forEach(([part, child]) => visit(child, key ? `${key}.${part}` : part));
    } else rows.push({ key, label: Object.hasOwn(CONFIG_LABELS, key) ? CONFIG_LABELS[key] : key, value: value === null ? SETTINGS.unset : String(value) });
  }
  visit(resolved, '');
  return rows.map(row => {
    if (row.key.startsWith('roles.')) {
      const roles = (resolved as { roles: { provenance?: string }[] }).roles;
      const source = ({ global: '全局', project: '项目', default: '默认' } as Record<string, string>)[roles[Number(row.key.split('.')[1])]?.provenance ?? ''];
      return source ? { ...row, source } : row;
    }
    return defaults.includes(row.key.split('.')[0]) ? { ...row, isDefault: true } : row;
  });
}
