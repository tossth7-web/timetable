import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { VitePWA } from 'vite-plugin-pwa'

// 部署在子路径下时（GitHub Pages 的 https://<user>.github.io/timetable/）由部署流程传
// VITE_BASE=/timetable/ 覆盖；桌面端、Android 壳、根路径部署（Cloudflare Pages）都用默认 '/'。
// manifest 的 start_url/scope 必须跟 base 对齐，否则 Chrome 判定不可安装。
const base = (globalThis as any).process?.env?.VITE_BASE || '/'

// Web 部署（默认 mode=production）：出 manifest + sw.js，手机可安装、可离线。
// 桌面端（TAURI_ENV_PLATFORM）和 Android 壳（--mode capacitor）：资源都是本地内嵌的，
// SW 在这里没有收益，只会拿上一次构建的预缓存糊住新 UI（踩过两次），所以不出。
export default defineConfig(({ mode }) => ({
  base,
  plugins: [
    react(),
    VitePWA({
      disable: mode === 'capacitor' || !!(globalThis as any).process?.env?.TAURI_ENV_PLATFORM,
      registerType: 'autoUpdate',
      includeAssets: ['icon.svg', 'pwa-192.png', 'pwa-512.png'],
      manifest: {
        name: '课表',
        short_name: '课表',
        description: '跨平台课表应用',
        start_url: base,
        scope: base,
        display: 'standalone',
        // 手机深色模式下启动画面会是浅色（manifest 只有一份，配色跟随 system 无法表达），
        // 取浅色主题的 --bg: #f6f7f9 最稳妥
        background_color: '#f6f7f9',
        theme_color: '#2563eb',
        lang: 'zh-CN',
        icons: [
          { src: 'pwa-192.png', sizes: '192x192', type: 'image/png', purpose: 'any' },
          { src: 'pwa-512.png', sizes: '512x512', type: 'image/png', purpose: 'any' },
          { src: 'icon.svg', sizes: 'any', type: 'image/svg+xml', purpose: 'any' },
        ],
      },
    }),
  ],
  clearScreen: false,
  server: { port: 5173, strictPort: false },
  build: { target: 'es2020' },
}))
