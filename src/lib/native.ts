import type { OcrResult } from './ocr'

/** 桌面端（Tauri）能力封装。Web/PWA 下全部静默降级，页面不用写两套逻辑。 */

export const IS_TAURI = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export async function pickImage(): Promise<string | null> {
  if (!IS_TAURI) return null
  const { open } = await import('@tauri-apps/plugin-dialog')
  const picked = await open({ multiple: false, filters: [{ name: '图片', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] })
  return typeof picked === 'string' ? picked : null
}

export async function saveText(defaultName: string, text: string): Promise<boolean> {
  if (!IS_TAURI) return false
  const { save } = await import('@tauri-apps/plugin-dialog')
  const path = await save({ defaultPath: defaultName, filters: [{ name: 'JSON', extensions: ['json'] }] })
  if (!path) return true // 用户取消，不算失败
  const { writeTextFile } = await import('@tauri-apps/plugin-fs')
  await writeTextFile(path, text)
  return true
}

/** 桌面端菜单项回调。菜单在 Tauri 启动时构建。 */
export function onMenuEvent(handler: (id: string) => void): () => void {
  if (!IS_TAURI) return () => {}
  let un: (() => void) | undefined
  import('@tauri-apps/api/event').then(({ listen }) => {
    listen<string>('menu', (e) => handler(e.payload)).then((f) => (un = f))
  })
  return () => un?.()
}

/** 预留：桌面端发 HTTP（未来教务导入用）。Rust 侧强制 https/http，超时 20s。 */
export async function httpGet(url: string): Promise<string> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<string>('http_get', { url })
}

/** 桌面端本地 OCR：把图片路径交给 Rust，Rust 调 127.0.0.1 的 Umi-OCR。 */
export async function ocrImage(path: string): Promise<OcrResult> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<OcrResult>('ocr_image', { path })
}

/** 打开内置教务窗口（Tauri 侧建 WebView）。已开着就聚焦。 */
export async function openAdmin(url: string): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('open_admin', { url })
}

/** 教务窗口里点「解析导入」后，页面 HTML 会以 admin-html 事件发过来 */
export function onAdminHtml(handler: (payload: { url: string; html: string }) => void): () => void {
  if (!IS_TAURI) return () => {}
  let un: (() => void) | undefined
  import('@tauri-apps/api/event').then(({ listen }) => {
    listen<{ url: string; html: string }>('admin-html', (e) => handler(e.payload)).then((f) => (un = f))
  })
  return () => un?.()
}
