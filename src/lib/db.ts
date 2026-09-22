import Dexie, { type Table } from 'dexie'
import type { AppData, Course, Settings } from './types'
import { asPeriods, DEFAULT_EVENING_FROM, DEFAULT_PERIODS } from './periods'
import { fmtDate } from './week'

export { DEFAULT_PERIODS, EVENING_CALL } from './periods'

export const DEFAULT_SETTINGS: Settings = {
  semesterStart: fmtDate(new Date()),
  periods: DEFAULT_PERIODS,
  globalReminder: '上课记得带书',
  eveningFrom: DEFAULT_EVENING_FROM,
  allowCloudOcr: false,
}

class DB extends Dexie {
  courses!: Table<Course, number>
  settings!: Table<{ key: string; value: unknown }, string>
  constructor() {
    super('timetable')
    this.version(1).stores({ courses: '++id, day', settings: 'key' })
  }
}

export const db = new DB()

/** 旧数据兼容：periods 可能是 ["08:20-09:05"] 这种字符串数组 / 结构化对象数组 */
export async function getSettings(): Promise<Settings> {
  const row = await db.settings.get('app')
  const saved = (row?.value ?? {}) as Partial<Settings>
  return { ...DEFAULT_SETTINGS, ...saved, periods: asPeriods(saved.periods) ?? DEFAULT_SETTINGS.periods }
}

export async function getAllCourses(): Promise<Course[]> {
  const list = await db.courses.toArray()
  return list.sort((a, b) => a.day - b.day || a.start - b.start)
}

export async function saveCourse(c: Course): Promise<void> {
  await db.courses.put(c)
}

export async function deleteCourse(id: number): Promise<void> {
  await db.courses.delete(id)
}

export async function getData(): Promise<AppData> {
  return { courses: await getAllCourses(), settings: await getSettings() }
}

/** 覆盖式写入全部数据（用于恢复备份）。带 id 的按 id 覆盖，不带的自增。 */
export async function putAll(data: AppData): Promise<void> {
  await db.transaction('rw', db.courses, db.settings, async () => {
    await db.courses.clear()
    await db.courses.bulkPut(data.courses)
    // 旧备份里 periods 是字符串数组，onboarding 时统一成结构化形态
    const periods = asPeriods((data.settings as Partial<Settings>)?.periods) ?? DEFAULT_SETTINGS.periods
    await db.settings.put({ key: 'app', value: { ...DEFAULT_SETTINGS, ...data.settings, periods } })
  })
}

export async function saveSettings(s: Settings): Promise<void> {
  await db.settings.put({ key: 'app', value: s })
}
