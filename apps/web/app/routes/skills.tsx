import { Card } from '../components/ui/card.tsx';
import { Ribbon } from '../components/ui/ribbon.tsx';
import { TEST_IDS } from '../lib/testids.ts';
import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';
export const handle = { crumb: '技能' };
export const loader = () => null;
export const shouldRevalidate = liveShouldRevalidate(byKind('config.', 'project.'));
export default function Page() { return <div data-testid={TEST_IDS.pageSkills}><Ribbon color="green">技能</Ribbon><Card color="green" patterned><h1>技能</h1><p>这里暂时没有可用内容。</p></Card></div>; }
