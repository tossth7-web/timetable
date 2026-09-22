# 课表 · Timetable

本地存储的跨平台课表：Windows 桌面端（Tauri 2）+ Web/PWA。数据只存在你自己的机器上，不上传、不登录、无账号。

> English README coming soon.

![周课表](docs/screenshot-timetable.png)

## 功能

- **跨平台**：Windows 桌面端（Tauri 2 打包的 exe / NSIS 安装包）+ 浏览器 PWA（可加到主屏幕，离线可用）
- **手动添加课程**：课名、教师、地点、备注、星期、节次、周次、颜色；保存前检测同周同时段冲突
- **AI 识图导入**：桌面端走**本机** Umi-OCR（图片不出机器）；Web 端默认本地模拟，开启上传后才发给你自己配的 OCR 服务
- **教务系统导入**：内置 WebView 打开教务系统，点页面上的按钮抓当前课表页 HTML，由 Rust 本地解析成课程；支持金智教育 `jwapp` 课表结构（另有一条通用表格兜底路径）
- **周次**：`1-16`、`1-8单`、`1-8双`、`1,3,5`、单周 `17`；上一周 / 下一周 / 回到本周，按学期开始日期算，今天高亮
- **自定义节次时间**：每节课的起止时间可增删改，晚自习单独一行并高亮
- **课程备注 + 全局提醒**：备注显示在色块右上角；主页顶部一句可自定义的提醒
- **本地存储**：IndexedDB（Dexie），整份数据可导出/导入 JSON 备份

![教务系统导入](docs/screenshot-import.png)

## 技术栈

| 层 | 用了什么 |
|---|---|
| 桌面外壳 | Tauri 2（Rust） |
| 前端 | React 18 + Vite 5 + TypeScript，无 UI 框架、无 CDN/字体/图片外链 |
| 存储 | IndexedDB（Dexie 4） |
| 后端命令 | Rust：`ocr_image`（本机 Umi-OCR）、`parse_zhengfang`（课表 HTML 解析）、`open_admin`（内置教务窗口）、`http_get` |
| 测试 | `node --test`（前端纯函数）+ `cargo test --lib`（解析器） |

## 开发

```bash
npm install
npm run dev        # http://localhost:5173
```

跑测试：

```bash
npm test                     # 前端：周次/周数、OCR 行解析、默认时间表 —— 21 个用例
npx tsc --noEmit             # 类型检查
cd src-tauri && cargo test --lib   # Rust：课表解析器 —— 16 个用例
```

目录：

```
src/                前端（pages / components / lib，无 UI 框架）
src-tauri/src/      Rust：lib.rs（命令）、admin.rs（课表 HTML 解析器）
test/               node --test 用例
scripts/            构建脚本
```

## 构建

**Web / PWA**

```bash
npm run build      # -> dist/（含 sw.js，可离线）
npm run preview    # 本地预览构建产物
```

**Windows 桌面端**

```powershell
pwsh scripts/tauri-build.ps1 -RunTests
```

脚本会：关掉正在运行的旧 exe（否则链接会失败）→ 前端构建并校验 `dist/` 完整 → `cargo test --lib` → `tauri build` → 校验 exe 里确实嵌入了当前的入口 JS。产物：

- 绿色版：`src-tauri/target/release/timetable.exe`
- 安装包：`src-tauri/target/release/bundle/nsis/课表_0.1.0_x64-setup.exe`

需要 MSVC 构建工具链（脚本会自己注入 `vcvars64.bat`）。

**macOS / Linux**：本仓库目前**没有**打包好的 dmg / AppImage，需要在各自平台上自行构建（`npm run tauri build`）。见下方「已知限制」。

## 部署 PWA

纯静态，把 `dist/` 丢给任意静态托管即可：

- **Cloudflare Pages**：Build command `npm run build`，Output directory `dist`
- **Vercel**：Framework 选 Vite，Build command / Output 同上

## 隐私

- 课表、设置只存在本机浏览器的 IndexedDB 里。没有账号、没有云端、没有统计埋点。
- 无外部 CDN / 字体 / 图片，装好后可断网使用。
- **识图**：桌面端图片只发给本机 `127.0.0.1:1224` 的 Umi-OCR，不出机器；Web 端默认不上传，只有手动打开开关才会把图片作为请求体发给你配置的 OCR 服务。
- **教务导入**：学号密码只在应用内置的 WebView 窗口里输入，程序不读取、不保存、不上传；抓到的 HTML 只用于本机解析，解析完即丢，全程没有本程序发起的网络请求。
- 清理浏览器数据 / 卸载应用 / 换设备都会丢数据且无法找回，请定期导出 JSON 备份。

## 免责声明

- 课表数据**以学校教务系统为准**：解析是启发式的，识别结果需要你在导入前自己核对。
- 本项目与任何学校、教务系统厂商无关，未获得其授权或背书。
- 请遵守你所在学校的信息系统使用规定，不要用它做批量抓取或对教务系统施压。

## 已知限制

1. **教务解析目前只覆盖金智教育 `jwapp` 课表结构**（已在真实教务页面上校准）；另有一条通用/正方样式的表格兜底路径，但那条路径**没有在真实页面上验证过**。
2. **macOS / Linux 未打包**：仓库里没有 dmg / AppImage / deb，需要在该平台自行构建。
3. **Web 端不支持教务导入**：浏览器跨域拿不到教务页面，只能用桌面端导入或粘贴 HTML。
4. 合并单元格里塞多门课的表格、纯图形课表识别不出来；识别不了的行会列进「需人工确认」，不会静默丢。
5. OCR 只适配 Umi-OCR 的 `/api/ocr`（v2 / v3 都试过），没有多引擎适配。

## 开源协议

[MIT](LICENSE) © 2026 timetable contributors
