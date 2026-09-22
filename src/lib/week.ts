export const DAYS = ['周一', '周二', '周三', '周四', '周五', '周六', '周日']

/** [1..n]，给节次下拉用 */
export const periods = (n: number) => Array.from({ length: n }, (_, i) => i + 1)

/** "1-16" / "1-8单" / "1-8双" / "1,3,5" -> 周次数组（升序去重，限定 1..30） */
export function parseWeeks(text: string): number[] {
  const out = new Set<number>()
  for (const raw of text.split(/[,，、]/)) {
    const part = raw.trim()
    if (!part) continue
    const parity = part.endsWith('单') ? 1 : part.endsWith('双') ? 0 : -1
    const body = parity < 0 ? part : part.slice(0, -1).trim()
    const m = body.match(/^(\d+)\s*[-~—]\s*(\d+)$/)
    const range = m ? [+m[1], +m[2]] : body.match(/^\d+$/) ? [+body, +body] : null
    if (!range) continue
    for (let w = Math.min(...range); w <= Math.max(...range); w++) {
      if (w >= 1 && w <= 30 && (parity < 0 || w % 2 === parity)) out.add(w)
    }
  }
  return [...out].sort((a, b) => a - b)
}

const pad = (n: number) => String(n).padStart(2, '0')
export const fmtDate = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`

/** 把 yyyy-mm-dd 当本地日期解析（new Date('2024-01-01') 会按 UTC，跨时区会差一天） */
export function parseDate(s: string): Date | null {
  const m = s.match(/^(\d{4})-(\d{2})-(\d{2})$/)
  if (!m) return null
  return new Date(+m[1], +m[2] - 1, +m[3])
}

/** 某天的周一 */
export function weekStart(d: Date): Date {
  const monday = new Date(d)
  const shift = (d.getDay() + 6) % 7
  monday.setDate(d.getDate() - shift)
  monday.setHours(0, 0, 0, 0)
  return monday
}

/** 今天是第几周；学期还没开始返回 1 */
export function currentWeek(semesterStart: string, today = new Date()): number {
  const start = parseDate(semesterStart)
  if (!start) return 1
  const days = Math.floor((weekStart(today).getTime() - weekStart(start).getTime()) / 86400000)
  return Math.max(1, Math.floor(days / 7) + 1)
}

/** 第 n 周每一天的日期 */
export function weekDates(semesterStart: string, week: number): string[] {
  const start = parseDate(semesterStart) ?? new Date()
  const monday = weekStart(start)
  monday.setDate(monday.getDate() + (week - 1) * 7)
  return DAYS.map((_, i) => {
    const d = new Date(monday)
    d.setDate(monday.getDate() + i)
    return fmtDate(d)
  })
}
