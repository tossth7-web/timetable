import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  asPeriods,
  blankToUndefined,
  dayOf,
  DEFAULT_EVENING_FROM,
  EVENING_CALL,
  isEvening,
  DEFAULT_PERIODS,
  periodOf,
  periodTime,
} from '../src/lib/periods.ts'

test('默认时间表是 9 节：上午 4 + 下午 4 + 晚自习 1', () => {
  assert.equal(DEFAULT_PERIODS.length, 9)
  assert.deepEqual(
    DEFAULT_PERIODS.map(periodTime),
    [
      '08:20-09:05',
      '09:15-10:00',
      '10:20-11:05',
      '11:15-12:00',
      '14:00-14:45',
      '14:55-15:40',
      '16:00-16:45',
      '16:55-17:40',
      '18:30-20:30',
    ],
  )
})

test('只有第 9 节是晚自习，且点名时间早于上课', () => {
  const s = { eveningFrom: DEFAULT_EVENING_FROM }
  const flags = DEFAULT_PERIODS.map((p, i) => isEvening(p, i + 1, s))
  assert.deepEqual(flags, [false, false, false, false, false, false, false, false, true])
  assert.ok(EVENING_CALL < periodTime(DEFAULT_PERIODS[8]).split('-')[0])
})

test('eveningFrom 被用户改小时，前面的节次也算晚自习', () => {
  assert.equal(isEvening({ time: '16:00-16:45' }, 7, { eveningFrom: 7 }), true)
  assert.equal(isEvening({ time: '16:00-16:45' }, 6, { eveningFrom: 7 }), false)
})

test('旧数据的字符串 periods 能读回结构化形态', () => {
  assert.deepEqual(asPeriods(['08:00-08:45', '18:30-20:30']), [{ time: '08:00-08:45' }, { time: '18:30-20:30' }])
  assert.deepEqual(asPeriods([{ time: 'x', type: 'evening' }]), [{ time: 'x', type: 'evening' }])
  assert.equal(asPeriods([]), undefined)
  assert.equal(asPeriods(undefined), undefined)
  assert.equal(asPeriods(['']), undefined)
})

test('可选项收尾：空白备注存成 undefined，脏 day/节次被夹到合法范围', () => {
  assert.equal(blankToUndefined('  '), undefined)
  assert.equal(blankToUndefined('带电脑'), '带电脑')
  assert.equal(dayOf(0), 1)
  assert.equal(dayOf('9'), 1)
  assert.equal(dayOf(5), 5)
  assert.equal(periodOf(0), 1)
  assert.equal(periodOf('99'), 30)
})
