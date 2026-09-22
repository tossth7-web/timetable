import type { ReactNode } from 'react'

const TABS = [
  { href: '#/', label: '课表' },
  { href: '#/add', label: '添加' },
  { href: '#/import', label: '导入' },
  { href: '#/settings', label: '设置' },
  { href: '#/about', label: '关于' },
]

export default function Layout({ path, children }: { path: string; children: ReactNode }) {
  const active = TABS.filter((t) => path.startsWith(t.href.slice(1))).pop() ?? TABS[0]

  const links = () =>
    TABS.map((t) => (
      <a key={t.href} href={t.href} className={t === active ? 'on' : ''}>
        {t.label}
      </a>
    ))

  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">课表</span>
        <nav className="tabs">{links()}</nav>
      </header>
      <main>{children}</main>
      <nav className="tabbar">{links()}</nav>
    </div>
  )
}
