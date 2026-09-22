import type { Course } from './types'
import { IS_TAURI } from './native'

/** 与 Rust src/admin.rs 的 ParseScheduleResult 同形状 */
export type ParseScheduleResult = { courses: Course[]; unparsed: string[]; debug: string[] }

/** 贴进来的东西像不像课表 HTML —— 只是快速提示，真正的判断在 Rust 解析器里 */
export const looksLikeHtml = (text: string) => /<\s*(table|tr|td|div|html)\b/i.test(text)

/**
 * 把 HTML 交给 Rust 解析（桌面端）。Web 端没有这个命令，返回 undefined 由页面提示。
 * 解析规则只有一份，在 src-tauri/src/admin.rs，有 cargo test 覆盖。
 */
export async function parseScheduleHtml(html: string): Promise<ParseScheduleResult | undefined> {
  if (!IS_TAURI) return undefined
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<ParseScheduleResult>('parse_zhengfang', { html })
}

/**
 * 解析结果 -> 人话。
 * ponytail: 故意不做「解析失败就猜」的补救（按列位置推断星期那套），猜错了会往课表里灌脏数据。
 */
export function summarizeParse(r: ParseScheduleResult): { ok: boolean; message: string } {
  if (r.courses.length) {
    return {
      ok: true,
      message: `从教务页面解析出 ${r.courses.length} 门课${r.unparsed.length ? `，另有 ${r.unparsed.length} 行没认出来（见下方）` : ''}`,
    }
  }
  return {
    ok: false,
    message: r.unparsed[0] ?? '没解析出课程：确认页面停在「我的课表」，或改用手动粘贴 HTML',
  }
}