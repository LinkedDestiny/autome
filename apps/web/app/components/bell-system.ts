import { levelOf } from '../../server/notifier-rules.ts';
import type { LiveMessage } from '../lib/live-client.ts';
import type { NotificationItem } from './bell-client.ts';
import { resolveTarget, TARGETS } from './bell-targets.ts';

export type SystemPermission = NotificationPermission | 'unsupported';
export type SystemNotice = { id: string; title: string; body?: string; href: string | null };
export type SystemNotifier = {
  permission(): SystemPermission;
  request(): Promise<SystemPermission>;
  show(notice: SystemNotice): void;
};
type Browser = { autome?: { notify?: (input: { title: string; body: string; href: string | null }) => Promise<unknown> }; Notification?: typeof Notification; focus(): void; location: { assign(href: string): void } };
export function browserSystemNotifier(browser: Browser): SystemNotifier {
  if (typeof browser.autome?.notify === 'function') {
    const native = browser.autome.notify.bind(browser.autome);
    return { permission: () => 'granted', request: async () => 'granted',
      show: ({ title, body, href }) => { void native({ title, body: body ?? '', href }).catch(error => console.error('系统通知失败', error)); } };
  }
  return {
    permission: () => browser.Notification?.permission ?? 'unsupported',
    request: async () => browser.Notification ? browser.Notification.requestPermission() : 'unsupported',
    show({ id, title, body, href }) {
      if (!browser.Notification) return;
      const notice = new browser.Notification(title, { body, tag: id });
      notice.onclick = () => {
        browser.focus();
        if (href !== null) browser.location.assign(href);
      };
    },
  };
}
export function shouldShow({ message, permission, hidden }: { message: LiveMessage; permission: SystemPermission; hidden: boolean }) {
  return levelOf(String(message.kind)) === 'need_you' && permission === 'granted' && hidden;
}
export function showSystemNotification(notifier: SystemNotifier, hidden: boolean, message: LiveMessage, items: NotificationItem[] | null, targets = TARGETS) {
  if (!shouldShow({ message, permission: notifier.permission(), hidden })) return;
  const item = items?.find(item => item.id === message.id);
  const { href } = resolveTarget(item ?? { task_id: message.task_id as string | null, project_id: null }, targets);
  notifier.show({ id: message.id as string, title: message.title as string, ...(item?.body ? { body: item.body } : {}), href });
}
