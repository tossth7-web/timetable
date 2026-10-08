import { useEffect, useRef, useState } from 'react'
import { db, DEFAULT_PERIODS, EVENING_CALL, getData, getSettings, saveSettings } from '../lib/db'
import { download, exportJson, restore } from '../lib/backup'
import { IS_TAURI, saveText } from '../lib/native'
import { DEFAULT_EVENING_FROM } from '../lib/periods'
import { isEvening } from '../lib/timetable'
import { parseDate } from '../lib/week'
import type { Settings } from '../lib/types'

const TIME_RE = /^([01]\d|2[0-3]):[0-5]\d-([01]\d|2[0-3]):[0-5]\d$/

export default function SettingsPage() {
  const [s, setS] = useState<Settings | null>(null)
  const [msg, setMsg] = useState('')
  const [pasteText, setPasteText] = useState('')
  const fileRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    getSettings().then(setS)
  }, [])

  if (!s) return <p className="empty">加载中…</p>

  const update = (patch: Partial<Settings>) => setS({ ...s, ...patch })

  const save = async () => {
    if (!parseDate(s.semesterStart)) return setMsg('学期开始日期格式应为 2025-09-01')
    const bad = s.periods.findIndex((t) => !TIME_RE.test(t.time))
    if (bad >= 0) return setMsg(`第 ${bad + 1} 节时间格式应为 08:20-09:05`)
    await saveSettings(s)
    setMsg('已保存')
  }

  const doExport = async () => {
    const text = await exportJson()
    const name = `课表备份-${new Date().toISOString().slice(0, 10)}.json`
    if (IS_TAURI && (await saveText(name, text))) return setMsg(`已导出 ${name}`)
    download(name, text)
    setMsg('浏览器已开始下载备份文件')
  }

  /** 选文件导入与粘贴文本导入走同一条恢复逻辑 */
  const applyBackup = async (text: string) => {
    try {
      const n = await restore(text)
      setS(await getSettings())
      setMsg(`已导入 ${n} 门课（覆盖原有课表）`)
      return true
    } catch (e) {
      setMsg('导入失败：' + (e as Error).message)
      return false
    }
  }

  const doImport = async (file: File) => {
    await applyBackup(await file.text())
  }

  const doImportPaste = async () => {
    const text = pasteText.trim()
    if (!text) return setMsg('先把备份 JSON 粘进来，再点「导入粘贴的内容」')
    if (await applyBackup(text)) setPasteText('')
  }

  const clearAll = async () => {
    if (!confirm('清空本机全部课程？此操作不可撤销，建议先导出备份。')) return
    const { courses } = await getData()
    await db.courses.bulkDelete(courses.flatMap((c) => (c.id != null ? [c.id] : [])))
    setMsg('已清空课程')
  }

  return (
    <div className="card">
      <h2>设置</h2>

      <label>
        学期开始日期（第 1 周的周一）
        <input type="date" value={s.semesterStart} onChange={(e) => update({ semesterStart: e.target.value })} />
      </label>

      <h3>每节课时间</h3>
      {s.periods.map((p, i) => {
        const night = isEvening(p, i + 1, s)
        return (
          <div className={'period-row' + (night ? ' night' : '')} key={i}>
            <span className="idx">
              第 {i + 1} 节{night ? '（晚自习）' : ''}
            </span>
            <input
              value={p.time}
              placeholder="08:20-09:05"
              onChange={(e) =>
                update({ periods: s.periods.map((x, j) => (j === i ? { ...x, time: e.target.value } : x)) })
              }
            />
            <button
              className="icon"
              aria-label="删除这一节"
              onClick={() => update({ periods: s.periods.filter((_, j) => j !== i) })}
            >
              ✕
            </button>
          </div>
        )
      })}
      {s.periods.some((p, i) => isEvening(p, i + 1, s)) && (
        <p className="hint">晚自习提醒：{EVENING_CALL} 前到教室点名。</p>
      )}
      <div className="row">
        <button className="btn" onClick={() => update({ periods: [...s.periods, { time: '08:20-09:05' }] })}>
          添加一节
        </button>
        <label className="inline">
          第
          <input
            type="number"
            min={1}
            max={s.periods.length}
            value={s.eveningFrom}
            onChange={(e) => update({ eveningFrom: Math.max(1, +e.target.value || 1) })}
          />
          节起算晚自习
        </label>
      </div>
      <div className="row">
        <button
          className="btn"
          onClick={() => {
            if (!confirm('用默认时间表覆盖当前 9 节时间？当前手动改过的时间会丢失（课程数据不受影响）。')) return
            update({ periods: DEFAULT_PERIODS, eveningFrom: DEFAULT_EVENING_FROM })
            setMsg('已恢复默认时间表，点「保存设置」写入本机')
          }}
        >
          恢复默认时间表
        </button>
        <span className="hint">覆盖本地保存的旧时间表；只动时间，不动课程</span>
      </div>

      <h3>全局提醒</h3>
      <label>
        课表主页顶部显示（留空则不显示）
        <input
          value={s.globalReminder}
          onChange={(e) => update({ globalReminder: e.target.value })}
          placeholder="上课记得带书"
        />
      </label>

      <div className="row">
        <button className="btn primary" onClick={save}>
          保存设置
        </button>
      </div>

      <h3>识图隐私</h3>
      {IS_TAURI ? (
        <p className="hint">桌面端走本机 OCR（Umi-OCR，127.0.0.1:1224），图片不出机器，无需开关。</p>
      ) : (
        <label className="pick">
          <input
            type="checkbox"
            checked={s.allowCloudOcr}
            onChange={(e) => {
              update({ allowCloudOcr: e.target.checked })
              setMsg(e.target.checked ? '已允许上传图片识别，记得点「保存设置」' : '已关闭上传，识图将使用本地模拟结果')
            }}
          />
          允许把图片上传到 OCR 服务识别（默认关闭）
        </label>
      )}
      {!IS_TAURI && <p className="hint">关闭时识图只在本地返回模拟结果，不发起任何网络请求；打开后需要构建时配置 VITE_OCR_ENDPOINT。</p>}

      <h3>备份</h3>
      <div className="row">
        <button className="btn" onClick={doExport}>
          导出 JSON
        </button>
        <button className="btn" onClick={() => fileRef.current?.click()}>
          导入 JSON
        </button>
        <button className="btn danger" onClick={clearAll}>
          清空课程
        </button>
      </div>
      <input
        ref={fileRef}
        type="file"
        accept="application/json,.json"
        hidden
        onChange={(e) => {
          const f = e.target.files?.[0]
          if (f) doImport(f)
          e.target.value = ''
        }}
      />

      <details className="pastejson">
        <summary>或者直接粘贴备份 JSON 文本</summary>
        <p className="hint">
          手机上不方便传文件时用这个：在电脑上打开备份文件全选复制，发到手机（微信/QQ 发给自己都行），
          在下面长按粘贴，再点「导入粘贴的内容」。粘贴的内容只在这台设备的这个框里，不会上传。
        </p>
        <textarea
          className="jsonpaste"
          value={pasteText}
          onChange={(e) => setPasteText(e.target.value)}
          placeholder={'把备份 JSON 全文粘到这里（以 {"app":"timetable" 开头）'}
          rows={5}
        />
        <div className="row">
          <button className="btn" onClick={doImportPaste}>
            导入粘贴的内容
          </button>
          {!!pasteText && (
            <button className="btn" onClick={() => setPasteText('')}>
              清空粘贴框
            </button>
          )}
        </div>
      </details>

      {msg && <p className="notice">{msg}</p>}
      <p className="hint">备份是明文 JSON，含全部课程与设置；数据只在本机，不会上传。</p>
    </div>
  )
}
