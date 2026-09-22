import type { Period, Settings } from './types'

const t = (time: string, type?: 'evening'): Period => (type ? { time, type } : { time })

/**
 * 默认作息：上午 4 节 + 下午 4 节 + 晚自习 1 节（18:30-20:30）。
 * 已有数据不会被覆盖：getSettings() 以库里存的值优先，这里只在首次初始化时生效。
 */
export const DEFAULT_PERIODS: Period[] = [
  t('08:20-09:05'),
  t('09:15-10:00'),
  t('10:20-11:05'),
  t('11:15-12:00'),
  t('14:00-14:45'),
  t('14:55-15:40'),
  t('16:00-16:45'),
  t('16:55-17:40'),
  t('18:30-20:30', 'evening'),
]

/** 晚自习到教室点名的时间（只用于提示，不参与排课） */
export const EVENING_CALL = '18:20'

/** 默认从第几节起算晚自习 */
export const DEFAULT_EVENING_FROM = 9

/** 旧版把 periods 存成字符串数组，这里兼容两种形态 */
export const periodTime = (p: Period | string) => (typeof p === 'string' ? p : p.time)

/** 是否晚自习：按节次下标（1-based）与 eveningFrom 比，同时兼容 type 标记 */
export const isEvening = (p: Period | string | undefined, index1: number, s: Pick<Settings, 'eveningFrom'>) =>
  (typeof p === 'object' && p?.type === 'evening') || index1 >= (s.eveningFrom || Number.MAX_SAFE_INTEGER)

/** 任意形态的 periods（旧字符串数组 / 新对象数组）统一成 Period[]；空/无效返回 undefined */
export const asPeriods = (v: unknown): Period[] | undefined => {
  if (!Array.isArray(v)) return undefined
  const list = v.map((p) => (typeof p === 'string' ? { time: p } : (p as Period))).filter((p) => !!p?.time)
  return list.length ? list : undefined
}

/** 非空字符串，否则 undefined（可选字段的收尾用） */
export const blankToUndefined = (v: string | undefined) => (v?.trim() ? v.trim() : undefined)

export const dayOf = (v: unknown): number => {
  const d = Math.round(Number(v))
  return Number.isFinite(d) && d >= 1 && d <= 7 ? d : 1
}

/** 节次收尾：1..30 的整数，否则 1 */
export const periodOf = (v: unknown): number => {
  const p = Math.round(Number(v))
  return Number.isFinite(p) ? Math.min(30, Math.max(1, p)) : 1
}
