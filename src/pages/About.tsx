import { IS_TAURI } from '../lib/native'

export default function About() {
  return (
    <div className="card prose">
      <h2>关于与隐私</h2>

      <h3>这个应用做什么</h3>
      <p>一款大学生周课表工具：记录课程、按周查看、导入导出备份。手机端是 PWA（浏览器「添加到主屏幕」），电脑端是 Tauri 打包的桌面软件，同一套 React 代码。</p>

      <h3>隐私</h3>
      <ul>
        <li>课表数据只存在你自己的设备上（浏览器/客户端的 IndexedDB），不上传、不联网同步。</li>
        <li>不收集个人信息，不埋点，不接第三方统计，不要求登录。</li>
        <li>没有外部 CDN、外部字体、外部图片：装好之后断网也能用。</li>
      </ul>

      <h3>识图的图片去哪了</h3>
      {IS_TAURI ? (
        <ul>
          <li>
            <b>桌面端：图片不出机器。</b>识别走本机运行的 Umi-OCR（<code>http://127.0.0.1:1224</code>），
            Rust 侧把图片读成 base64 后只发给本机服务，不经过任何外部网络，也不写入磁盘副本。
          </li>
          <li>没启动 Umi-OCR 时不会偷偷改用云服务，会直接报「未检测到本地 OCR 服务」。</li>
        </ul>
      ) : (
        <ul>
          <li>
            <b>Web 端：默认不上传。</b>「允许上传图片到 OCR 服务识别」默认关闭；关闭时识图只在本地返回模拟结果，
            不发起任何网络请求。
          </li>
          <li>
            只有你手动打开该开关（设置页），图片才会作为识别请求的请求体发给你配置的 OCR 服务（构建时变量
            <code>VITE_OCR_ENDPOINT</code>）。开关状态随备份一起导出。
          </li>
          <li>无论开关如何，课表本身始终只存本地。</li>
        </ul>
      )}

      <h3>免责声明</h3>
      <p>数据仅存本地。清理浏览器数据、卸载应用、更换设备都会导致课表丢失，且无法找回。请定期用「设置 → 导出 JSON」备份。因数据丢失造成的后果由使用者自行承担。</p>

      <h3>技术栈</h3>
      <p>React + Vite + TypeScript；存储 Dexie（IndexedDB）；PWA 用 vite-plugin-pwa；桌面端 Tauri 2 + 本地 OCR（Umi-OCR）；样式为原生 CSS + CSS Grid，无 UI 框架。</p>

      <h3>当前运行环境</h3>
      <p>
        {IS_TAURI
          ? '桌面端（Tauri）：本地 OCR、文件对话框、导出保存对话框、菜单栏。'
          : '浏览器 / PWA：导入导出走浏览器下载与文件选择框；识图默认使用本地模拟结果。'}
      </p>

      <h3>后续计划</h3>
      <ul>
        <li>按学校补教务系统解析（桌面端走 Rust <code>http_get</code>，手机端走 WebView 注入）。</li>
        <li>可选：课程提醒、桌面小组件。</li>
      </ul>

      <p className="hint">版本 0.1.0 · 本地优先，够用就好。</p>
    </div>
  )
}
