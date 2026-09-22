import { useEffect, useState } from 'react'
import { deleteCourse, getAllCourses, getSettings } from '../lib/db'
import type { Course, Settings } from '../lib/types'
import { DAYS, currentWeek, fmtDate, weekDates } from '../lib/week'
import { EVENING_CALL } from '../lib/db'
import { colorOf, isEvening, periodRange, periodTime, weeksSummary } from '../lib/timetable'
import Sheet from '../components/Sheet'

export default function Schedule() {
  const [courses, setCourses] = useState<Course[]>([])
  const [settings, setSettings] = useState<Settings | null>(null)
  const [week, setWeek] = useState(() => Number(localStorage.getItem('week')) || 0)
  const [ready, setReady] = useState(false)
  const [detail, setDetail] = useState<Course | null>(null)

  useEffect(() => {
    ;(async () => {
      const [cs, st] = await Promise.all([getAllCourses(), getSettings()])
      const w = week || currentWeek(st.semesterStart)
      setCourses(cs)
      setSettings(st)
      setWeek(w)
      setReady(true)
    })()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const changeWeek = (w: number) => {
    setWeek(w)
    localStorage.setItem('week', String(w))
  }

  const remove = async (c: Course) => {
    if (!confirm(`删除「${c.name}」？`)) return
    if (c.id != null) await deleteCourse(c.id)
    setCourses(await getAllCourses())
    setDetail(null)
  }

  if (!ready || !settings) return <p className="empty">加载中…</p>

  const dates = weekDates(settings.semesterStart, week)
  const shown = courses.filter((c) => c.weeks.includes(week))
  const maxPeriod = Math.max(settings.periods.length, ...courses.map((c) => c.end), 1)
  const today = fmtDate(new Date())
  /** 第 i+2 行是第 i+1 节：第 1 行固定给星期表头 */
  const list = Array.from({ length: maxPeriod }, (_, i) => {
    const p = settings.periods[i]
    return {
      time: p ? periodTime(p) : '',
      night: isEvening(p, i + 1, settings),
      missing: !p,
    }
  })

  return (
    <div className="schedule">
      {settings.globalReminder?.trim() && <p className="reminder">〔提醒〕{settings.globalReminder.trim()}</p>}

      <div className="weekbar">
        <div className="nav">
          <button onClick={() => changeWeek(Math.max(1, week - 1))} aria-label="上一周">
            ‹
          </button>
          <select value={week} onChange={(e) => changeWeek(+e.target.value)} aria-label="选择周次">
            {Array.from({ length: Math.max(30, week) }, (_, i) => i + 1).map((w) => (
              <option key={w} value={w}>
                第 {w} 周
              </option>
            ))}
          </select>
          <button onClick={() => changeWeek(week + 1)} aria-label="下一周">
            ›
          </button>
          <button className="ghost" onClick={() => changeWeek(currentWeek(settings.semesterStart))}>
            回到本周
          </button>
        </div>
        <span className="count">{shown.length} 门课</span>
      </div>

      <div
        className="grid"
        style={
          {
            '--period-rows': list.map((p) => (p.night ? 'var(--night-h)' : 'var(--period-h)')).join(' '),
          } as React.CSSProperties
        }
      >
        <div className="corner cell">节次</div>
        {DAYS.map((d, i) => (
          <div key={d} className={'head cell' + (dates[i] === today ? ' today' : '')}>
            <b>{d}</b>
            <small>{dates[i].slice(5)}</small>
          </div>
        ))}

        {list.map((p, i) => (
          <div
            key={i}
            className={'axis cell' + (p.night ? ' night' : '') + (p.missing ? ' missing' : '')}
            style={{ gridRow: i + 2 }}
          >
            <b>
              {i + 1}
              {p.night && <em>晚</em>}
            </b>
            {p.time
              ? p.time.split('-').map((hm) => <small key={hm}>{hm}</small>)
              : <small>未设置</small>}
            {p.night && <small className="call">{EVENING_CALL} 点名</small>}
          </div>
        ))}

        {DAYS.map((d, i) => (
          <div
            key={d}
            className={'col cell' + (dates[i] === today ? ' today' : '')}
            style={{ gridColumn: i + 2, gridRow: `2 / ${maxPeriod + 2}` }}
          >
            {shown
              .filter((c) => c.day === i + 1)
              .map((c) => (
                <button
                  key={c.id}
                  className="course"
                  style={{ gridRow: `${c.start} / ${c.end + 1}`, background: colorOf(c) }}
                  onClick={() => setDetail(c)}
                >
                  <b>{c.name}</b>
                  {c.room && <small>@{c.room}</small>}
                  {c.teacher && <small>{c.teacher}</small>}
                  {c.note && (
                    <span className="note-dot" title={`备注：${c.note}`} aria-label={`备注：${c.note}`}>
                      ✎
                    </span>
                  )}
                </button>
              ))}
          </div>
        ))}
      </div>

      <p className="hint">手机上可左右滑动查看整周 · 点色块看详情</p>

      <Sheet open={!!detail} title={detail?.name ?? ''} onClose={() => setDetail(null)}>
        {detail && (
          <>
            <dl className="detail">
              <dt>教师</dt>
              <dd>{detail.teacher || '—'}</dd>
              <dt>地点</dt>
              <dd>{detail.room || '—'}</dd>
              <dt>时间</dt>
              <dd>
                {DAYS[detail.day - 1]} {periodRange(detail)}
              </dd>
              <dt>周次</dt>
              <dd>{weeksSummary(detail)}</dd>
              {detail.note && (
                <>
                  <dt>备注</dt>
                  <dd>{detail.note}</dd>
                </>
              )}
            </dl>
            <div className="row">
              <a className="btn" href={`#/add/${detail.id}`}>
                编辑
              </a>
              <button className="btn danger" onClick={() => remove(detail)}>
                删除
              </button>
            </div>
          </>
        )}
      </Sheet>
    </div>
  )
}
