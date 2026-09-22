import { DAYS } from './week'
import type { Course } from './types'

export { isEvening, periodTime } from './periods'

export const COLORS = [
  '#2563eb',
  '#0891b2',
  '#059669',
  '#ca8a04',
  '#ea580c',
  '#dc2626',
  '#db2777',
  '#7c3aed',
]

export const colorOf = (c: Course) => COLORS[c.color % COLORS.length]

/** 找同一时段已有课程，返回重叠的那门（用于添加时提示冲突） */
// ponytail: 只按传入的那个周次判重叠（添加时用首个上课周），跨周次的精细冲突不管。
export function findConflict(courses: Course[], c: Course, week: number): Course | undefined {
  return courses.find(
    (o) =>
      o.id !== c.id &&
      o.day === c.day &&
      o.weeks.includes(week) &&
      c.weeks.includes(week) &&
      o.start <= c.end &&
      o.end >= c.start,
  )
}

export const weeksSummary = (c: Course) => c.weeksText || (c.weeks.length ? `${c.weeks[0]}-${c.weeks[c.weeks.length - 1]}` : '')

export const periodRange = (c: Course) => `第 ${c.start}-${c.end} 节`
