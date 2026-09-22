import { useEffect, useRef, useState } from 'react'
import { getAllCourses, getSettings, saveCourse } from '../lib/db'
import { LOW_CONFIDENCE, lowCourses, needsReview, recognizeImage, toDataUrl } from '../lib/importers'
import type { OcrUnparsed } from '../lib/importers'
import { looksLikeHtml, parseScheduleHtml, summarizeParse, type ParseScheduleResult } from '../lib/adminImport'
import { DAYS, parseWeeks, periods } from '../lib/week'
import { COLORS, findConflict } from '../lib/timetable'
import type { Course, Settings } from '../lib/types'
import { IS_TAURI, onAdminHtml, openAdmin, pickImage } from '../lib/native'

/** 未解析行的展示形态：OCR 给 {text, confidence}，教务 HTML 给纯字符串 */
const asLine = (u: OcrUnparsed | string): OcrUnparsed =>
  typeof u === 'string' ? { text: u, confidence: 0.3 } : u

export default function ImportPage() {
  const [settings, setSettings] = useState<Settings | null>(null)
  const [preview, setPreview] = useState('')
  const [busy, setBusy] = useState(false)
  const [elapsed, setElapsed] = useState(0)
  const [drafts, setDrafts] = useState<Course[]>([])
  const [unparsed, setUnparsed] = useState<OcrUnparsed[]>([])
  const [picked, setPicked] = useState<Set<number>>(new Set())
  const [msg, setMsg] = useState('')
  const [htmlPaste, setHtmlPaste] = useState('')
  const [adminBusy, setAdminBusy] = useState(false)
  const [adminDebug, setAdminDebug] = useState<string[]>([])
  const fileRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    getSettings().then(setSettings)
  }, [])

  // 教务窗口点「解析导入」后，HTML 会以 admin-html 事件发过来
  useEffect(
    () =>
      onAdminHtml(async ({ url, html }) => {
        setAdminBusy(false)
        if (url && /login|auth/i.test(url)) {
          setMsg('这个页面看起来还是登录页：请先在教务窗口里登录并打开「我的课表」，再点右下角按钮')
          return
        }
        await runParse(html)
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
  )

  // 进度提示：识别期间的秒数（超时上限 35 秒）
  useEffect(() => {
    if (!busy) return
    setElapsed(0)
    const t = setInterval(() => setElapsed((n) => n + 1), 1000)
    return () => clearInterval(t)
  }, [busy])

  /** 解析结果灌进预览：与 OCR 走同一套 drafts/unparsed */
  const applyResult = (res: ParseScheduleResult) => {
    const list = res.courses.map((c, i) => ({ ...c, color: c.color ?? i % COLORS.length }))
    setDrafts(list)
    setUnparsed(res.unparsed.map(asLine))
    setAdminDebug(res.debug ?? [])
    setPicked(new Set(list.map((_, i) => i)))
    setPreview('')
    setMsg(summarizeParse(res).message)
  }

  const runParse = async (html: string) => {
    if (!html.trim()) return setMsg('还没有内容：先打开教务窗口并点右下角按钮，或把课表页 HTML 粘到下面')
    const res = await parseScheduleHtml(html)
    if (!res) return setMsg('浏览器里没有本地解析器：请用桌面版，或改用「设置 → 导入 JSON」')
    applyResult(res)
  }

  const parseHtml = async (html: string) => {
    if (!html.trim()) return setMsg('先把课表页的 HTML 粘到下面的框里')
    if (!looksLikeHtml(html)) return setMsg('这段内容不像 HTML：需要整页源码（含 <table> 或 <html>）')
    setMsg('解析中…')
    await runParse(html)
  }

  const openAdminWindow = async () => {
    const url = prompt('学校教务系统地址（含 http:// 或 https://）', 'https://')
    if (!url) return
    try {
      await openAdmin(url.trim())
      setAdminBusy(true)
      setMsg('教务窗口已打开：登录后进到「我的课表」，点窗口右下角的「解析导入到课表」')
    } catch (e) {
      setAdminBusy(false)
      setMsg('打不开教务窗口：' + (e as Error).message)
    }
  }

  const recognize = async (src: File | string) => {
    setBusy(true)
    setMsg('')
    setUnparsed([])
    try {
      if (src instanceof File) setPreview(await toDataUrl(src))
      else setPreview('')
      const res = await recognizeImage(src)
      const list = res.courses.map((c, i) => ({ ...c, color: c.color ?? i % COLORS.length }))
      setDrafts(list)
      setUnparsed(res.unparsed)
      setPicked(new Set(list.map((_, i) => i)))
      if (!list.length && !res.unparsed.length) {
        setMsg('没认出课程：图里没有「星期 + 节次」这种信息，请换张更清晰的课表截图，或手动添加')
        return
      }
      const low = lowCourses(res).length
      setMsg(`识别出 ${list.length} 门课${low ? `，其中 ${low} 门置信度偏低（标黄）` : ''}`)
    } catch (e) {
      setDrafts([])
      setUnparsed([])
      setMsg('识别失败：' + (e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const pickDesktopImage = async () => {
    const path = await pickImage()
    if (path) await recognize(path)
  }

  const patch = (i: number, p: Partial<Course>) => setDrafts((d) => d.map((x, j) => (j === i ? { ...x, ...p } : x)))

  const toggle = (i: number) =>
    setPicked((p) => {
      const next = new Set(p)
      if (!next.delete(i)) next.add(i)
      return next
    })

  /** 删掉一门草稿后，勾选下标要跟着左移 */
  const dropDraft = (i: number) => {
    setDrafts((x) => x.filter((_, j) => j !== i))
    setPicked((p) => new Set([...p].filter((j) => j !== i).map((j) => (j > i ? j - 1 : j))))
  }

  /**
   * 汇总条：跳到第一条需要人工确认的行。
   * 低置信度课程排在未解析行前面，选择器命中顺序就是跳转优先级。
   */
  const jumpToFirst = () =>
    document.querySelector('.draft.low, [id^=unparsed-]')?.scrollIntoView({ behavior: 'smooth', block: 'center' })

  const importAll = async () => {
    if (!picked.size) return setMsg('先勾选要导入的课程')
    let n = 0
    const clashes: string[] = []
    const existing = await getAllCourses()
    for (const i of picked) {
      const d = drafts[i]
      if (!d) continue
      const weeks = parseWeeks(d.weeksText)
      if (!d.name.trim() || !weeks.length) continue
      const course: Course = {
        ...d,
        id: undefined,
        name: d.name.trim(),
        start: Math.min(d.start, d.end),
        end: Math.max(d.start, d.end),
        weeks,
      }
      if (findConflict(existing, course, weeks[0])) clashes.push(d.name)
      await saveCourse(course)
      existing.push(course)
      n++
    }
    setDrafts([])
    setUnparsed([])
    setPicked(new Set())
    setPreview('')
    setMsg(`已导入 ${n} 门课${clashes.length ? `（${clashes.join('、')} 与已有课程时间重叠，请检查）` : ''}`)
  }

  const n = settings ? settings.periods.length : 12
  const mode = IS_TAURI ? '桌面端本地 OCR（Umi-OCR，图片不出机器）' : settings?.allowCloudOcr ? '上传到 OCR 服务识别' : '本地模拟识别（未开启上传）'
  const lowCount = lowCourses({ courses: drafts, unparsed }).length
  const reviewCount = needsReview({ courses: drafts, unparsed })

  return (
    <div className="card">
      <h2>导入课表</h2>

      <h3>手动添加</h3>
      <p className="hint">
        一门一门填请走 <a href="#/add">添加课程</a>。
      </p>

      <h3>AI 识图导入</h3>
      <div className="row">
        <button className="btn primary" disabled={busy} onClick={() => fileRef.current?.click()}>
          {busy ? `识别中… ${elapsed}s` : '选择图片'}
        </button>
        {IS_TAURI && (
          <button className="btn" disabled={busy} onClick={pickDesktopImage}>
            桌面端文件对话框
          </button>
        )}
      </div>
      <input
        ref={fileRef}
        type="file"
        accept="image/*"
        hidden
        onChange={(e) => {
          const f = e.target.files?.[0]
          if (f) recognize(f)
          e.target.value = ''
        }}
      />
      <p className="hint">当前识别方式：{mode}。单次识别超过 35 秒会超时。</p>
      {IS_TAURI && <p className="hint">桌面端需要先启动 Umi-OCR（默认 http://127.0.0.1:1224），否则会提示「未检测到本地 OCR 服务」。</p>}

      {preview && <img className="preview" src={preview} alt="待识别截图预览" />}

      {reviewCount > 0 && (
        <button className="notice warnbar" onClick={jumpToFirst}>
          以下 {reviewCount} 行识别置信度较低，请手动确认
          {lowCount > 0 && unparsed.length > 0 ? `（${lowCount} 门课程 + ${unparsed.length} 行未解析）` : ''}
          <span className="jump">跳到第一条 ↓</span>
        </button>
      )}

      {!!drafts.length && (
        <div className="drafts">
          <h3>识别结果（可编辑）</h3>
          {drafts.map((d, i) => {
            const low = (d.confidence ?? 1) < LOW_CONFIDENCE
            return (
              <div
                className={'draft' + (picked.has(i) ? '' : ' off') + (low ? ' low' : '')}
                key={i}
                id={`draft-${i}`}
              >
                {low && <p className="warn">置信度 {Math.round((d.confidence ?? 0) * 100)}%，请人工确认这一行</p>}
                <label className="pick">
                  <input type="checkbox" checked={picked.has(i)} onChange={() => toggle(i)} />
                  <input value={d.name} onChange={(e) => patch(i, { name: e.target.value })} placeholder="课程名" />
                </label>
                <div className="two">
                  <input value={d.teacher} onChange={(e) => patch(i, { teacher: e.target.value })} placeholder="教师" />
                  <input value={d.room} onChange={(e) => patch(i, { room: e.target.value })} placeholder="地点" />
                </div>
                <div className="two">
                  <select value={d.day} onChange={(e) => patch(i, { day: +e.target.value })}>
                    {DAYS.map((day, j) => (
                      <option key={day} value={j + 1}>
                        {day}
                      </option>
                    ))}
                  </select>
                  <div className="two">
                    <select value={d.start} onChange={(e) => patch(i, { start: +e.target.value })}>
                      {periods(n).map((p) => (
                        <option key={p} value={p}>
                          {p}
                        </option>
                      ))}
                    </select>
                    <select value={d.end} onChange={(e) => patch(i, { end: +e.target.value })}>
                      {periods(n).map((p) => (
                        <option key={p} value={p}>
                          {p}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>
                <div className="two">
                  <input value={d.weeksText} onChange={(e) => patch(i, { weeksText: e.target.value })} placeholder="1-16" />
                  <span className="colors">
                    {COLORS.map((c, j) => (
                      <button
                        type="button"
                        key={c}
                        className={'swatch' + (d.color === j ? ' on' : '')}
                        style={{ background: c }}
                        aria-label={`颜色 ${j + 1}`}
                        onClick={() => patch(i, { color: j })}
                      />
                    ))}
                  </span>
                </div>
                <button className="btn danger small" onClick={() => dropDraft(i)}>
                  删除这条
                </button>
              </div>
            )
          })}
          <div className="row">
            <button className="btn primary" onClick={importAll}>
              批量导入（{picked.size}）
            </button>
            <button
              className="btn"
              onClick={() => {
                setDrafts([])
                setUnparsed([])
                setPicked(new Set())
              }}
            >
              全部放弃
            </button>
          </div>
        </div>
      )}

      {!!unparsed.length && (
        <div className="drafts">
          <h3>需人工确认（{unparsed.length} 行未能识别成课程）</h3>
          {unparsed.map((u, i) => (
            <div className="draft low" key={i} id={`unparsed-${i}`}>
              <p className="warn">这行认不出「星期 + 节次」，原文如下</p>
              <p className="raw">{u.text}</p>
              <div className="row">
                <a className="btn small" href="#/add">
                  照着手动添加
                </a>
                <button className="btn danger small" onClick={() => setUnparsed((x) => x.filter((_, j) => j !== i))}>
                  忽略这行
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <h3>从教务系统导入</h3>
      {IS_TAURI ? (
        <>
          <p className="hint">
            打开内置窗口登录教务系统，进到「我的课表」页面后，点窗口右下角蓝色的「解析导入到课表」。
            账号密码只在那个窗口里用，我们不读取、不保存、不上传。
          </p>
          <div className="row">
            <button className="btn primary" disabled={adminBusy} onClick={openAdminWindow}>
              {adminBusy ? '已打开教务窗口…' : '打开教务窗口'}
            </button>
            <button className="btn" disabled={adminBusy} onClick={() => parseHtml(htmlPaste)}>
              解析粘贴的 HTML
            </button>
          </div>
          {adminBusy && <p className="hint">登录后翻到课表页，点窗口右下角的「解析导入到课表」就会回到这里。</p>}
        </>
      ) : (
        <p className="hint">
          浏览器里没法开内置窗口（跨域拿不到教务页面）。桌面版可用；Web 端可把课表页的 HTML 粘贴到下面，
          或改用「设置 → 导入 JSON」。
        </p>
      )}

      <textarea
        className="htmlpaste"
        value={htmlPaste}
        onChange={(e) => setHtmlPaste(e.target.value)}
        placeholder="解析失败时，把教务课表页面的 HTML 源码粘贴到这里（浏览器里 Ctrl+U 或 F12 → 右键 <html> → Copy outerHTML），再点「解析粘贴的 HTML」。"
        rows={4}
      />

      {!!adminDebug.length && (
        <details className="hint">
          <summary>解析诊断（{adminDebug.length} 条）</summary>
          <ul className="debuglist">
            {adminDebug.map((d, i) => (
              <li key={i}>{d}</li>
            ))}
          </ul>
        </details>
      )}

      {msg && <p className="notice">{msg}</p>}
    </div>
  )
}
