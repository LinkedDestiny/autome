import { Links, Meta, Outlet, Scripts, ScrollRestoration } from "react-router";
export function Layout({ children }: { children: React.ReactNode }) {
  return (<html lang="zh-CN"><head><meta charSet="utf-8" /><title>Autome 面板</title><Meta /><Links /></head><body>{children}<ScrollRestoration /><Scripts /></body></html>);
}
export default function App() { return <Outlet />; }
