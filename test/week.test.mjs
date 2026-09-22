import assert from 'node:assert/strict'
import { test } from 'node:test'
// ponytail: 直接跑 TS 源码自检，不为一个纯函数装 vitest。Node 24 原生剥类型。
import { currentWeek, parseWeeks, weekDates } from '../src/lib/week.ts'

test('parseWeeks 连续区间', () => {
  assert.deepEqual(parseWeeks('1-16'), Array.from({ length: 16 }, (_, i) => i + 1))
})

test('parseWeeks 单双周', () => {
  assert.deepEqual(parseWeeks('1-8单'), [1, 3, 5, 7])
  assert.deepEqual(parseWeeks('2-8双'), [2, 4, 6, 8])
  assert.deepEqual(parseWeeks('1-16双'), [2, 4, 6, 8, 10, 12, 14, 16])
})

test('parseWeeks 离散与混合', () => {
  assert.deepEqual(parseWeeks('1,3,5'), [1, 3, 5])
  assert.deepEqual(parseWeeks('1-4, 6, 9-10'), [1, 2, 3, 4, 6, 9, 10])
  assert.deepEqual(parseWeeks('3'), [3])
})

test('parseWeeks 垃圾输入不炸', () => {
  assert.deepEqual(parseWeeks(''), [])
  assert.deepEqual(parseWeeks('abc'), [])
  assert.deepEqual(parseWeeks('单'), [])
  assert.deepEqual(parseWeeks('0-3'), [1, 2, 3])
  assert.deepEqual(parseWeeks('1-99'), Array.from({ length: 30 }, (_, i) => i + 1))
})

test('currentWeek 边界', () => {
  assert.equal(currentWeek('2025-09-01', new Date(2025, 8, 1)), 1) // 学期第一天
  assert.equal(currentWeek('2025-09-01', new Date(2025, 8, 7)), 1) // 周日仍算第 1 周
  assert.equal(currentWeek('2025-09-01', new Date(2025, 8, 8)), 2) // 下周一
  assert.equal(currentWeek('2025-09-01', new Date(2025, 7, 20)), 1) // 开学前 -> 1
  assert.equal(currentWeek('2025-09-01', new Date(2025, 11, 22)), 17) // 第 16 周之后=第 17 周
})

test('weekDates 一周 7 天，第 1 周从学期开始日算起', () => {
  assert.deepEqual(weekDates('2025-09-01', 1), [
    '2025-09-01',
    '2025-09-02',
    '2025-09-03',
    '2025-09-04',
    '2025-09-05',
    '2025-09-06',
    '2025-09-07',
  ])
  assert.equal(weekDates('2025-09-01', 3)[0], '2025-09-15')
})
