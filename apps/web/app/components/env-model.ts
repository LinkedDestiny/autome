import { ENV } from '../lib/admin-labels.ts';
export type Login = { login: 'not_applicable' | 'ok' | 'expired' | 'unknown'; account_hint?: string | null; detail?: string };
export type EnvComponent = { component: string; present: boolean; version: string | null; path: string | null; login: Login };
export type EnvSnapshot = { probed: boolean; probing: boolean; generation: number; environment: { checked_at?: string; components: EnvComponent[] } | null; problems: { component: string; name?: string; present: boolean; login: Login }[] };
export type InstallRecipe = { recipe: { component: string; command: string; prerequisite: string | null; note: string | null }; prerequisite_ok: boolean; login_command: string | null };
export type EnvData = { kernel?: import("../../server/instance.ts").Instance["kernel"]; kernelError?: string; drivers?: import('../../server/drivers.ts').DriverStatus; snapshot?: EnvSnapshot; recipes: Record<string, InstallRecipe>; errors: Record<string, string>; error?: string };
export type EnvResult = { error?: string; launched?: boolean; command?: string; reason?: string };
export function loginLabel(login: Login): string {
  switch (login.login) {
    case 'not_applicable': return ENV.noLogin;
    case 'ok': return ENV.loggedIn + (login.account_hint ? `（${login.account_hint}）` : '');
    case 'expired': return ENV.expired;
    case 'unknown': return ENV.unknown + (login.detail ? `：${login.detail}` : '');
  }
}
export function environmentNotice(value: unknown): string | null {
  const env = value as Pick<EnvSnapshot, 'probed' | 'problems'> | undefined;
  if (!env?.probed || !env.problems?.length) return null;
  return ENV.summary(env.problems.length) + env.problems.map(p => `${p.name ?? p.component}（${p.present ? loginLabel(p.login) : ENV.missing}）`).join('、');
}

export function toolOk(component: EnvComponent, problems: EnvSnapshot['problems']): boolean {
  return component.present && ['ok', 'not_applicable'].includes(component.login.login) && !problems.some(p => p.component === component.component);
}
