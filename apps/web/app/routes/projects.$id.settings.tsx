import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';
export const shouldRevalidate = liveShouldRevalidate(byKind('project.'));
import { redirect, type LoaderFunctionArgs } from 'react-router';
export function loader({ params }: LoaderFunctionArgs) {
  return redirect('/loop?project=' + encodeURIComponent(params.id ?? '') + '&tab=project', 302);
}
