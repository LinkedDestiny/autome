import { useEffect } from 'react';
import { useRevalidator } from 'react-router';
import { subscribeLiveMessages } from '../lib/live-client.ts';
import { revalidateAll } from '../lib/live-routes.ts';

export function revalidatesOnMessage(type: string) {
  return type === 'notification' || type === 'notification-read';
}
export function subscribeNotificationRevalidate(subscribe: typeof subscribeLiveMessages, revalidate: () => void | Promise<void>) {
  return subscribe(type => { if (revalidatesOnMessage(type)) void revalidateAll(revalidate); });
}
export function useNotificationRevalidate() {
  const { revalidate } = useRevalidator();
  useEffect(() => subscribeNotificationRevalidate(subscribeLiveMessages, revalidate), [revalidate]);
}
