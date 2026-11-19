import { useEffect, useState } from 'react';
import { Link } from 'react-router';
import type { DriverStatus } from '../../server/drivers.ts';
import { subscribeLiveMessages } from '../lib/live-client.ts';
import { useClock } from '../lib/clock.tsx';
import { driverCardModel } from './driver-model.ts';
export function DriverCard({ initial }: { initial?: DriverStatus }) {
  const [live, setLive] = useState<DriverStatus>();
  useEffect(() => { setLive(undefined); }, [initial]);
  useEffect(() => subscribeLiveMessages((type, message) => { if (type === 'drivers') setLive(message.status as DriverStatus); }), []);
  const model = driverCardModel(live ?? initial, useClock().now);
  return <section className="driver-card" aria-label="驱动状态"><span className={model.active ? 'driver-dot active' : 'driver-dot'} /><div><strong>{model.title}</strong>{model.detail && <small>{model.detail}</small>}{model.title === '没有驱动在运行' && <Link className="btn btn-text btn-sm" to="/env">查看环境</Link>}</div></section>;
}
