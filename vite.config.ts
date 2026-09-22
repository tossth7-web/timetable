import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { VitePWA } from 'vite-plugin-pwa'

export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      // 桌面端资源是内嵌的，SW 只会拿上一次构建的预缓存糊住新 UI（改完代码仍显示旧页面）。
      // Tauri 只在 tauri build/dev 的子进程里设 TAURI_ENV_PLATFORM，所以 Web PWA 不受影响。
      // ponytail: 没装 @types/node，读 env 就不为一行判断引一个依赖；哪天要在桌面端做离线更新再打开
      disable: !!(globalThis as any).process?.env?.TAURI_ENV_PLATFORM,
      registerType: 'autoUpdate',
      includeAssets: ['icon.svg'],
      manifest: {
        name: '课表',
        short_name: '课表',
        description: '本地存储的大学生课表',
        start_url: '/',
        display: 'standalone',
        background_color: '#ffffff',
        theme_color: '#2563eb',
        icons: [{ src: 'icon.svg', sizes: 'any', type: 'image/svg+xml', purpose: 'any' }],
      },
    }),
  ],
  clearScreen: false,
  server: { port: 5173, strictPort: false },
  build: { target: 'es2020' },
})
