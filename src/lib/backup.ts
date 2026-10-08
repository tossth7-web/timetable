import type { Course } from './types'
import { getData, putAll } from './db'

const isCourse = (v: unknown): v is Course =>
  !!v && typeof v === 'object' && typeof (v as Course).name === 'string' && Array.isArray((v as Course).weeks)

/** 导出为 JSON 字符串 */
export async function exportJson(): Promise<string> {
  const { courses, settings } = await getData()
  return JSON.stringify({ app: 'timetable', version: 1, exportedAt: new Date().toISOString(), courses, settings }, null, 2)
}

/** 读取备份文件内容，兼容纯数组 */
export async function parseBackup(text: string) {
  let raw: unknown
  try {
    raw = JSON.parse(text)
  } catch {
    throw new Error('不是合法的 JSON（可能没粘全，或被聊天软件截断/加料）')
  }
  const obj = Array.isArray(raw) ? { courses: raw } : (raw as Record<string, unknown>)
  if (!Array.isArray(obj.courses) || !obj.courses.every(isCourse)) throw new Error('文件里没有可识别的课程数据')
  return {
    courses: obj.courses as Course[],
    settings: (obj.settings ?? null) as Awaited<ReturnType<typeof getData>>['settings'] | null,
  }
}

/** 导入备份（覆盖）。settings 为 null 时保留当前设置。 */
export async function restore(text: string): Promise<number> {
  const { courses, settings } = await parseBackup(text)
  const current = await getData()
  await putAll({ courses, settings: settings ?? current.settings })
  return courses.length
}

/** 触发浏览器下载；桌面端由原生对话框接管（见 pages/Settings.tsx） */
export function download(filename: string, text: string) {
  const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }))
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}
