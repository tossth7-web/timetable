import type { Course } from './types'
// 带 .ts 后缀是给 `node --test` 用的（Vite 也认）
import { parseWeeks } from './week.ts'

/** 低于这个置信度就在预览里标黄，归入「需人工确认」 */
export const LOW_CONFIDENCE = 0.7

/** 一行 OCR 文本，没能解析成课程；保留原文让人工确认 */
export type OcrUnparsed = { text: string; confidence: number }

/** 识别结果：能解析成课程的 + 需要人工确认的未解析行 */
export type OcrResult = { courses: Course[]; unparsed: OcrUnparsed[] }

/** 低于阈值的课程（预览里标黄的那些） */
export const lowCourses = (r: OcrResult) => r.courses.filter((c) => (c.confidence ?? 1) < LOW_CONFIDENCE)

/** 需要人工确认的行数（低置信度课程 + 未解析行） */
export const needsReview = (r: OcrResult) => lowCourses(r).length + r.unparsed.length

/** 纯文本 -> 识别结果（Web 端识别服务只回文本时用）。桌面端的同名解析在 src-tauri/src/lib.rs。 */
export function recognizeOcrText(text: string): OcrResult {
  const courses: Course[] = []
  const unparsed: OcrUnparsed[] = []
  for (const line of text.split('\n')) {
    const trimmed = line.trim()
    const course = parseOcrLine(trimmed, undefined)
    if (course) courses.push(course)
    else if (trimmed) unparsed.push({ text: trimmed, confidence: 0.3 })
  }
  return { courses, unparsed }
}

const CJK_NUM = '一二三四五六日天'
const cjkNum = (c: string) => {
  const i = CJK_NUM.indexOf(c)
  return i < 0 ? undefined : i === 6 ? 7 : i + 1
}

/** 一行 OCR 文本 -> 一门课；与 Rust 侧 parse_line 同一套规则，拿不到星期或节次就返回 undefined */
export function parseOcrLine(text: string, score: number | undefined): Course | undefined {
  const day = (() => {
    for (const key of ['周', '星期', '礼拜']) {
      let from = 0
      for (;;) {
        const i = text.indexOf(key, from)
        if (i < 0) break
        const c = text[i + key.length]
        const d = /[1-7]/.test(c ?? '') ? Number(c) : cjkNum(c ?? '')
        if (d) return d
        from = i + key.length
      }
    }
    return undefined
  })()

  const span = (() => {
    for (let i = text.indexOf('节'); i >= 0; i = text.indexOf('节', i + 1)) {
      const token = text.slice(0, i).match(/[\d\-~第\s]+$/)?.[0] ?? ''
      const nums = token
        .trim()
        .replace(/^第/, '')
        .trim()
        .split(/[-~]/)
        .map((x) => Number(x.trim()))
        .filter((n) => Number.isInteger(n) && n > 0)
      if (nums.length) return [nums[0], nums[nums.length - 1]] as [number, number]
    }
    return undefined
  })()

  if (!day || !span) return undefined
  const [start, end] = [Math.min(...span), Math.max(...span)]

  const toks = text.trim().split(/\s+/)
  const used = new Set<number>()
  const take = (i: number, v: string) => {
    used.add(i)
    return v
  }

  const roomIdx = toks.findIndex((t) => /[教楼室馆区]/.test(t))
  const room = roomIdx >= 0 ? take(roomIdx, toks[roomIdx]) : ''

  let weeksText = '1-16'
  let weeks: number[] = []
  // 周次 token：1-8单 / 1,3,5 / 1-16周 —— 整块只有数字和分隔符，避免把“3-4节”当周次
  const weekIdx = toks.findIndex((t) => /^[\d,\-~、，]*\d[\d,\-~、，]*[周单双]?$/.test(t))
  if (weekIdx >= 0) {
    const stripped = take(weekIdx, toks[weekIdx]).replace(/周$/, '')
    const parsed = parseWeeks(stripped)
    if (parsed.length) {
      weeksText = stripped
      weeks = parsed
    }
  }
  const inferredWeeks = weeks.length === 0
  if (inferredWeeks) weeks = Array.from({ length: 16 }, (_, i) => i + 1)

  // 教师名按 ≤3 字判断：4 字的“高等数学”是课名，不是老师
  const teacherIdx = toks.findIndex((t, i) => !used.has(i) && t.length <= 3 && /^[\u4e00-\u9fff]+$/.test(t))
  const teacher = teacherIdx >= 0 ? take(teacherIdx, toks[teacherIdx]) : ''

  const nameIdx = toks.findIndex((t, i) => !used.has(i) && t.length > 1)
  const name = nameIdx >= 0 ? toks[nameIdx].replace(/^[\d.]+/, '') : text.trim()

  let confidence = score ?? 0.6
  if (!room) confidence -= 0.1
  if (inferredWeeks) confidence -= 0.15
  if (!teacher) confidence -= 0.05

  return {
    name: name || text.trim(),
    teacher,
    room,
    day,
    start,
    end,
    weeks,
    weeksText,
    color: 0,
    confidence: Math.max(0, Math.min(1, confidence)),
  }
}
