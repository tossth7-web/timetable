import assert from 'node:assert/strict'
import { test } from 'node:test'
// 与 src-tauri/src/lib.rs 的 parse_line / parse_lines 同一套规则，两边各有自检
// ponytail: .mjs 里不能写 TS 非空断言，靠 assert 收窄
import {
  LOW_CONFIDENCE,
  lowCourses,
  needsReview,
  parseOcrLine,
  recognizeOcrText,
} from '../src/lib/ocr.ts'

const ok = (line, score) => {
  const c = parseOcrLine(line, score)
  assert.ok(c, `应当解析出课程：${line}`)
  return c
}

test('parseOcrLine 正常一行', () => {
  const c = ok('高等数学 张伟 一教302 周一 1-2节 1-16周', 0.97)
  assert.equal(c.name, '高等数学')
  assert.equal(c.teacher, '张伟')
  assert.equal(c.room, '一教302')
  assert.deepEqual([c.day, c.start, c.end], [1, 1, 2])
  assert.equal(c.weeksText, '1-16')
  assert.equal(c.weeks.length, 16)
  assert.ok(c.confidence > LOW_CONFIDENCE)
})

test('parseOcrLine 中文星期三 + 单周', () => {
  const c = ok('大学英语 李娜 外语楼205 星期三 3-4节 1-8单')
  assert.deepEqual([c.day, c.start, c.end], [3, 3, 4])
  assert.deepEqual(c.weeks, [1, 3, 5, 7])
})

test('parseOcrLine 缺时间信息的行直接跳过', () => {
  assert.equal(parseOcrLine('2025-2026 学年第一学期课表', 0.99), undefined)
  assert.equal(parseOcrLine('周一 高数', 0.99), undefined)
  assert.equal(parseOcrLine('', 0.99), undefined)
})

test('parseOcrLine 没有周次时置信度下调', () => {
  const c = ok('数据结构 王强 计算机楼401 周五 5-6节', 0.95)
  assert.equal(c.weeksText, '1-16')
  assert.ok(c.confidence < 0.9)
})

test('阈值边界：0.69 低、0.70 不算低、0.71 不算低', () => {
  assert.equal(LOW_CONFIDENCE, 0.7)
  const line = '高等数学 张伟 一教302 周一 1-2节 1-16周'
  for (const [score, low] of [
    [0.69, true],
    [0.7, false],
    [0.71, false],
  ]) {
    const c = ok(line, score)
    assert.equal(c.confidence, score, `score=${score} 原样传递`)
    assert.equal(c.confidence < LOW_CONFIDENCE, low, `score=${score}`)
  }
})

test('解析成功的行 confidence 原样传递（含扣分后的值）', () => {
  // 全字段齐全 → 分数原样
  const full = ok('高等数学 张伟 一教302 周一 1-2节 1-16周', 0.88)
  assert.equal(full.confidence, 0.88)
  // 缺明确周次 -0.15
  const c = ok('数据结构 王强 计算机楼401 周五 5-6节', 0.95)
  assert.equal(Number(c.confidence.toFixed(2)), 0.8)
  // 缺地点 -0.1、缺周次 -0.15，分数不低于 0、不高于原值
  const d = ok('X 周一 1-2节', 0.5)
  assert.ok(d.confidence > 0 && d.confidence < 0.5, `confidence=${d.confidence}`)
  assert.equal(Number(d.confidence.toFixed(2)), 0.25)
})

test('无法解析的行归入 unparsed 并保留原文', () => {
  const r = recognizeOcrText('2025-2026 学年第一学期课表\n高等数学 张伟 一教302 周一 1-2节 1-16周\n周一 高数')
  assert.equal(r.courses.length, 1)
  assert.equal(r.courses[0].name, '高等数学')
  assert.deepEqual(
    r.unparsed.map((u) => u.text),
    ['2025-2026 学年第一学期课表', '周一 高数'],
  )
  assert.ok(r.unparsed.every((u) => u.confidence < LOW_CONFIDENCE))
})

test('recognizeOcrText 空行丢弃，课程与未解析行分流', () => {
  const r = recognizeOcrText('\n高等数学 张伟 一教302 周一 1-2节 1-16周\n\n标题')
  assert.equal(r.courses.length, 1)
  assert.equal(r.unparsed.length, 1)
  assert.equal(r.unparsed[0].text, '标题')
})

test('needsReview = 低置信度课程 + 未解析行；lowCourses 只挑低置信度课程', () => {
  const line = '高等数学 张伟 一教302 周一 1-2节 1-16周'
  const r = recognizeOcrText([line, line, '标题'].join('\n'))
  assert.equal(r.courses.length, 2)
  // 纯文本输入默认 0.6 → 两门都算低置信度，再加 1 行未解析
  assert.equal(needsReview(r), 3)
  assert.equal(lowCourses(r).length, 2)
  assert.equal(needsReview(r) - r.unparsed.length, lowCourses(r).length)
})

test('recognizeOcrText().courses 等价于旧的“只取课程”用法', () => {
  const list = recognizeOcrText('课程表\n高等数学 张伟 一教302 周一 1-2节 1-16周\n数据结构 王强 计算机楼401 周五 5-6节').courses
  assert.equal(list.length, 2)
  assert.deepEqual(
    list.map((c) => c.name),
    ['高等数学', '数据结构'],
  )
})
