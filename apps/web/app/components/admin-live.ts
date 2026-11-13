import { useEffect } from 'react';
import { useRevalidator } from 'react-router';
import { subscribeLiveMessages } from '../lib/live-client.ts';
import { revalidateAll } from '../lib/live-routes.ts';
export function subscribeAdminRevalidate(subscribe: typeof subscribeLiveMessages, revalidate: () => void | Promise<void>) {
  return subscribe(type => { if (type === 'onboarding.ended' || type === 'env.changed') void revalidateAll(revalidate); });
}
export function useAdminRevalidate() {
  const { revalidate } = useRevalidator();
  useEffect(() => subscribeAdminRevalidate(subscribeLiveMessages, revalidate), [revalidate]);
}
