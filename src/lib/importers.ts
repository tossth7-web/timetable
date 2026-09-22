import type { Course, Settings } from './types'
import { parseWeeks } from './week'
import { recognizeOcrText, type OcrResult } from './ocr'
import { IS_TAURI, ocrImage, pickImage } from './native'
import { getSettings } from './db'

export { LOW_CONFIDENCE, lowCourses, needsReview, parseOcrLine } from './ocr'
export type { OcrResult, OcrUnparsed } from './ocr'

/** 前端兜底超时；Rust 侧 30s 会先返回，这里只是别让界面卡死 */
const TIMEOUT_MS = 35_000

/** 模拟识图（Web 端默认走这条）：写死的 3 门课，接口形状与真实实现一致 */
async function mockRecognize(): Promise<OcrResult> {
  await new Promise((r) => setTimeout(r, 800))
  const rows: Array<[string, string, string, number, number, number, string, number, number]> = [
    ['高等数学 A', '张伟', '一教 302', 1, 3, 4, '1-16', 0, 1],
    ['大学英语（三）', '李娜', '外语楼 205', 3, 1, 2, '1-8单', 2, 1],
    // 故意给一行低置信度，方便在 Web 端看到标黄与汇总提示
    ['数据结构', '王强', '计算机楼 401', 5, 5, 6, '1-16', 7, 0.62],
  ]
  const courses = rows.map(([name, teacher, room, day, start, end, weeksText, color, confidence]) => ({
    name,
    teacher,
    room,
    day,
    start,
    end,
    weeksText,
    weeks: parseWeeks(weeksText),
    color,
    confidence,
  }))
  return { courses, unparsed: [] }
}

/** 读图片为 dataURL：Web 端用于本地预览，开启上传后才会作为请求体发给 OCR 服务 */
export const toDataUrl = (file: File) =>
  new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(file)
  })

/**
 * Web 端对外部 OCR 服务的调用。默认关闭（设置 → 允许上传图片识别），
 * 打开且配了地址才会发请求；否则抛错，由页面提示改用模拟或桌面端。
 * ponytail: 没做服务端实现，只留一个可配置的约定（POST {image: dataURL} -> {text: string}）。
 */
async function cloudRecognize(file: File, s: Settings): Promise<OcrResult> {
  if (!s.allowCloudOcr) throw new Error('未开启上传识别：设置 → 允许上传图片识别（默认关闭）')
  const endpoint = (import.meta.env.VITE_OCR_ENDPOINT as string | undefined) ?? ''
  if (!endpoint) throw new Error('未配置 OCR 服务地址（构建时设置 VITE_OCR_ENDPOINT）')
  const res = await fetch(endpoint, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ image: await toDataUrl(file) }),
  })
  if (!res.ok) throw new Error(`OCR 服务返回 HTTP ${res.status}`)
  const { text } = (await res.json()) as { text?: string }
  if (!text) throw new Error('OCR 服务没返回 text 字段')
  return recognizeOcrText(text)
}

/**
 * 统一入口：桌面端走本地 Umi-OCR（图片不出机器），Web 端走上传或本地模拟。
 * 返回 {courses, unparsed}，超时/失败都抛带中文说明的 Error。
 */
export async function recognizeImage(src: File | string): Promise<OcrResult> {
  const work = async (): Promise<OcrResult> => {
    if (typeof src === 'string') return ocrImage(src) // 桌面端：文件对话框给的是本地路径
    const s = await getSettings()
    if (IS_TAURI || s.allowCloudOcr) return cloudRecognize(src, s)
    return mockRecognize()
  }
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    return await Promise.race([
      work(),
      new Promise<OcrResult>((_, reject) => {
        timer = setTimeout(
          () => reject(new Error(`识别超时（${TIMEOUT_MS / 1000} 秒），请重试或换张更清晰的图`)),
          TIMEOUT_MS,
        )
      }),
    ])
  } finally {
    clearTimeout(timer)
  }
}
