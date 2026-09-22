import { useEffect, useState } from 'react'
import { getAllCourses, getSettings, saveCourse } from '../lib/db'
import { blankToUndefined, dayOf, periodOf } from '../lib/periods'
import { COLORS, findConflict } from '../lib/timetable'
import { DAYS, currentWeek, parseWeeks, periods } from '../lib/week'
import type { Course, Settings } from '../lib/types'

const blank = (): Course => ({
  name: '',
  teacher: '',
  room: '',
  day: 1,
  start: 1,
  end: 2,
  weeksText: '1-16',
  weeks: parseWeeks('1-16'),
  color: 0,
  note: '',
})

export default function AddCourse({ id }: { id?: number }) {
  const [form, setForm] = useState<Course>(blank)
  const [settings, setSettings] = useState<Settings | null>(null)
  const [msg, setMsg] = useState('')

  useEffect(() => {
    ;(async () => {
      setSettings(await getSettings())
      if (id) {
        const found = (await getAllCourses()).find((c) => c.id === id)
        if (found) setForm(found)
      }
    })()
  }, [id])

  const set = <K extends keyof Course>(k: K, v: Course[K]) => setForm((f) => ({ ...f, [k]: v }))
  const total = Math.max(settings?.periods.length ?? 12, form.end)

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    const name = form.name.trim()
    if (!name) return setMsg('课程名不能为空')
    const weeks = parseWeeks(form.weeksText)
    if (!weeks.length) return setMsg('周次看不懂，试试 “1-16” 或 “1-8单”')
    const course: Course = {
      ...form,
      name,
      note: blankToUndefined(form.note),
      day: dayOf(form.day),
      start: Math.min(periodOf(form.start), periodOf(form.end)),
      end: Math.max(periodOf(form.start), periodOf(form.end)),
      weeks,
    }

    const clash = findConflict(await getAllCourses(), course, weeks[0])
    if (clash && !confirm(`与「${clash.name}」（${DAYS[clash.day - 1]} 第 ${clash.start}-${clash.end} 节）时间重叠，仍然保存？`)) return

    await saveCourse(course)
    location.hash = '#/'
  }

  return (
    <form className="card" onSubmit={submit}>
      <h2>{id ? '编辑课程' : '添加课程'}</h2>

      <label>
        课程名
        <input value={form.name} onChange={(e) => set('name', e.target.value)} placeholder="高等数学 A" required />
      </label>

      <div className="two">
        <label>
          教师
          <input value={form.teacher} onChange={(e) => set('teacher', e.target.value)} placeholder="张伟" />
        </label>
        <label>
          地点
          <input value={form.room} onChange={(e) => set('room', e.target.value)} placeholder="一教 302" />
        </label>
      </div>

      <label>
        备注（可选）
        <input
          value={form.note ?? ''}
          onChange={(e) => set('note', e.target.value)}
          placeholder="带电脑 / 带书 / 实验课"
        />
      </label>

      <label>
        星期
        <select value={form.day} onChange={(e) => set('day', +e.target.value)}>
          {DAYS.map((d, i) => (
            <option key={d} value={i + 1}>
              {d}
            </option>
          ))}
        </select>
      </label>

      <div className="two">
        <label>
          开始节次
          <select value={form.start} onChange={(e) => set('start', +e.target.value)}>
            {periods(total).map((n) => (
              <option key={n} value={n}>
                第 {n} 节
              </option>
            ))}
          </select>
        </label>
        <label>
          结束节次
          <select value={form.end} onChange={(e) => set('end', +e.target.value)}>
            {periods(total).map((n) => (
              <option key={n} value={n}>
                第 {n} 节
              </option>
            ))}
          </select>
        </label>
      </div>

      <label>
        周次
        <input value={form.weeksText} onChange={(e) => set('weeksText', e.target.value)} placeholder="1-16 / 1-8单 / 1,3,5" />
      </label>
      <p className="hint">
        支持 <code>1-16</code>（连续）、<code>1-8单</code> / <code>1-8双</code>（单双周）、<code>1,3,5</code>（离散）
        {form.weeksText ? ` · 解析出 ${parseWeeks(form.weeksText).length} 周` : ''}
      </p>

      <label>
        颜色
        <span className="colors">
          {COLORS.map((c, i) => (
            <button
              type="button"
              key={c}
              className={'swatch' + (form.color === i ? ' on' : '')}
              style={{ background: c }}
              aria-label={`颜色 ${i + 1}`}
              onClick={() => set('color', i)}
            />
          ))}
        </span>
      </label>

      {msg && <p className="error">{msg}</p>}

      <div className="row">
        <button className="btn primary" type="submit">
          保存
        </button>
        <a className="btn" href="#/">
          取消
        </a>
      </div>
      {!id && settings && <p className="hint">当前是第 {currentWeek(settings.semesterStart)} 周</p>}
    </form>
  )
}
