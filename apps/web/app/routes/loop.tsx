import { Card } from '../components/ui/card.tsx';
import { Ribbon } from '../components/ui/ribbon.tsx';
import { TEST_IDS } from '../lib/testids.ts';
import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';
export const handle = { crumb: 'Loop 配置' };
export const loader = () => null;
export const shouldRevalidate = liveShouldRevalidate(byKind('config.', 'project.'));
export default function Page() { return <div data-testid={TEST_IDS.pageLoop}><Ribbon color="blue">Loop 配置</Ribbon><Card color="blue" patterned><h1>Loop 配置</h1><p>这里暂时没有可用内容。</p></Card></div>; }
