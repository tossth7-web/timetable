export type Course = {
  id?: number
  name: string
  teacher: string
  room: string
  day: number // 1=周一 ... 7=周日
  start: number // 开始节次
  end: number // 结束节次
  weeks: number[] // 上课周次
  weeksText: string // 原始输入，如 "1-8单"
  color: number // 调色板下标
  note?: string // 备注，如「带电脑」「实验课」
  confidence?: number // 识别置信度 0-1（手工添加的课程没有这个字段）
}

export type Period = {
  time: string // "08:20-09:05"
  type?: 'evening' // 特殊节次标记，当前只有晚自习
}

export type Settings = {
  semesterStart: string // YYYY-MM-DD
  periods: Period[]
  globalReminder: string // 主页顶部全局提醒，空串=不显示
  eveningFrom: number // 从第几节起算晚自习（默认 9）
  allowCloudOcr: boolean // Web 端是否允许把图片上传到 OCR 服务，默认 false
}

export type AppData = {
  courses: Course[]
  settings: Settings
}
