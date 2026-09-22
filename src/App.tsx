import { useEffect, useState } from 'react'
import Layout from './components/Layout'
import Schedule from './pages/Schedule'
import AddCourse from './pages/AddCourse'
import SettingsPage from './pages/Settings'
import ImportPage from './pages/Import'
import About from './pages/About'
import { IS_TAURI, onMenuEvent, saveText } from './lib/native'
import { exportJson } from './lib/backup'

export default function App() {
  const [path, setPath] = useState(() => location.hash.slice(1) || '/')

  useEffect(() => {
    const onHash = () => setPath(location.hash.slice(1) || '/')
    addEventListener('hashchange', onHash)
    return () => removeEventListener('hashchange', onHash)
  }, [])

  // 桌面端菜单：导出备份 / 导入备份 / 关于
  useEffect(() => {
    if (!IS_TAURI) return
    return onMenuEvent(async (id) => {
      if (id === 'export-backup') {
        await saveText(`课表备份-${new Date().toISOString().slice(0, 10)}.json`, await exportJson())
      } else if (id === 'import-backup') {
        location.hash = '#/settings'
      } else if (id === 'about') {
        location.hash = '#/about'
      }
    })
  }, [])

  const page = path.startsWith('/add') ? (
    <AddCourse id={Number(path.split('/')[2]) || undefined} />
  ) : path.startsWith('/settings') ? (
    <SettingsPage />
  ) : path.startsWith('/import') ? (
    <ImportPage />
  ) : path.startsWith('/about') ? (
    <About />
  ) : (
    <Schedule />
  )

  return <Layout path={path}>{page}</Layout>
}
