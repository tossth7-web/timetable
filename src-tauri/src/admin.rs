//! 教务系统课表 HTML 解析（优先适配新版正方）。
//!
//! 解析放在 Rust 侧的理由：能拿真实 HTML 片段做 `cargo test`，不依赖 WebView 组件。
//! 前端只负责提供 HTML（来自内置窗口或用户手动粘贴），结果走既有的预览/导入流程。

use serde::Serialize;
use std::sync::OnceLock;

// 复用 lib.rs 的 Course：字段与前端 src/lib/types.ts 一致
use crate::Course;

/// 下标 = 星期几（1..7）
const DAY_NAMES: [&str; 8] = ["", "周一", "周二", "周三", "周四", "周五", "周六", "周日"];

/// 解析结果：能用的课程 + 需要人工确认的行（贴进预览，格式与 OCR 导入一致）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseScheduleResult {
    pub courses: Vec<Course>,
    pub unparsed: Vec<String>,
    /// 诊断：抓到的表头列、每格解析成败。真实学校页面结构对不上时，靠它定位
    pub debug: Vec<String>,
}

/// 连续节次专用。真实页面的写法是 `第1节-第2节`（**第一个 `节` 之后**才是横线），
/// 也有 `第1-2节` 这种省略写法和 `1-2节`。所以第一个 `节?` 必须是可选的。
/// 字符类里把所有可能的横线都列上：ASCII `-`、`~`、U+2013 `–`、U+2014 `—`、U+FF0D `－`、U+2212 `−`。
fn period_span_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"第?\s*(\d{1,2})\s*节?\s*[-~–—－−]\s*第?\s*(\d{1,2})\s*节").unwrap())
}

/// 节次轴标签：连续「第1节-第2节」「第1-2节」与单节「第5节」都要认（判断行边界靠它）
fn period_label_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"第?\s*\d{1,2}\s*节?(?:\s*[-~–—－−]\s*第?\s*\d{1,2}\s*节)?").unwrap())
}

fn days_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(星期|周|礼拜)\s*([一二三四五六日天1-7])").unwrap())
}

fn span_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(\d{1,2})\s*[-~—]\s*(\d{1,2})").unwrap())
}

fn single_period_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?:^|[\s(（])第?\s*(\d{1,2})\s*节").unwrap())
}

/// 周次串：必须以「周」结尾，否则会把「第1节-第2节」里的 1-2 当成周次。
/// 前缀允许紧跟非数字（块内文本可能是 "B5103-18周" 这种黏连），但不能从数字中间起匹配。
fn weeks_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?:^|[^\d])(\d{1,2}(?:\s*[-~—]\s*\d{1,2})?(?:\s*[,，、]\s*\d{1,2}(?:\s*[-~—]\s*\d{1,2})?)*)\s*周\s*[（(]?\s*(单|双|odd|even)?\s*[)）]?").unwrap()
    })
}

fn cjk_num(c: char) -> Option<u32> {
    "一二三四五六日天".chars().position(|x| x == c).map(|i| if i == 6 { 7 } else { i as u32 + 1 })
}

fn day_of(s: &str) -> Option<u32> {
    let caps = days_re().captures(s)?;
    let c = caps.get(2)?.as_str().chars().next()?;
    match c {
        '1'..='7' => c.to_digit(10),
        _ => cjk_num(c),
    }
}

/// 去掉标签、解实体、压空白
fn text_of(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    // HTML 实体：足够覆盖正方页面，未收录的按原样保留（宁可多留字符也不丢内容）
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&emsp;", " ")
        .replace("&ensp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "1-8" / "11-13" / "3,5,7" / "17" / "1-16单" -> 周次数组（1..30，与前端 parseWeeks 同规则）
fn parse_weeks(s: &str) -> (Vec<u32>, String) {
    let mut out: Vec<u32> = Vec::new();
    let mut shown: Vec<String> = Vec::new();
    for part in s.split([',', '，', '、']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let parity = if part.ends_with('单') {
            1
        } else if part.ends_with('双') {
            0
        } else {
            -1
        };
        let body = part.trim_end_matches(['单', '双']).trim();
        let nums: Vec<u32> = body
            .split(['-', '~', '–', '—', '－', '−'])
            .filter_map(|x| x.trim().parse::<u32>().ok())
            .collect();
        match nums.as_slice() {
            [a, b] => {
                for w in (*a).min(*b)..=(*a).max(*b) {
                    if w >= 1 && w <= 30 && (parity < 0 || w % 2 == parity as u32) {
                        out.push(w);
                    }
                }
                shown.push(format!("{}-{}{}", a.min(b), a.max(b), if parity == 1 { "单" } else if parity == 0 { "双" } else { "" }));
            }
            [a] => {
                if *a >= 1 && *a <= 30 {
                    out.push(*a);
                }
                shown.push(a.to_string());
            }
            _ => {}
        }
    }
    out.sort_unstable();
    out.dedup();
    (out, shown.join(", "))
}

/// 周次 token -> (数组, 展示串, 去掉周次后剩下的文本)
fn weeks_of(token: &str) -> Option<(Vec<u32>, String, String)> {
    // 起点前面不能是数字：否则 "B5103-18周" 会被切成 "103-18周"
    let cap = weeks_re()
        .captures_iter(token)
        .find(|c| {
            c.get(0)
                .map(|m| {
                    let b = m.start();
                    b == 0 || !token.as_bytes()[b - 1].is_ascii_digit()
                })
                .unwrap_or(false)
        })?;
    let whole = cap.get(0)?;
    let body = cap.get(1).map(|m| m.as_str().replace(' ', "")).unwrap_or_default();
    let parity = cap.get(2).map(|m| m.as_str()).unwrap_or("");
    let input = format!(
        "{body}{}",
        match parity {
            "单" | "odd" => "单",
            "双" | "even" => "双",
            _ => "",
        }
    );
    let (mut weeks, text) = parse_weeks(&input);
    if weeks.is_empty() {
        return None;
    }
    if parity == "单" || parity == "双" {
        let want = if parity == "单" { 1 } else { 0 };
        weeks.retain(|w| w % 2 == want);
        if weeks.is_empty() {
            return None;
        }
    }
    let shown = if (parity == "单" || parity == "双") && !text.ends_with(parity) {
        format!("{text}{parity}")
    } else {
        text
    };
    let rest = format!("{} {}", &token[..whole.start()], &token[whole.end()..]);
    Some((weeks, shown, rest))
}

/// 节次 range：优先「第N节-第M节」，其次「N-M节」这种紧跟前缀的写法
fn period_range_after(t: &str, cut: usize) -> Option<(u32, u32)> {
    let tail = &t[cut.min(t.len())..];
    if let Some(c) = period_span_re().captures(tail) {
        return Some((c[1].parse().ok()?, c[2].parse().ok()?));
    }
    // N-M节（前面不接数字）
    for c in span_re().captures_iter(tail) {
        let m = c.get(0)?;
        if m.end() < tail.len() && tail[m.end()..].trim_start().starts_with('节') {
            return Some((c[1].parse().ok()?, c[2].parse().ok()?));
        }
    }
    None
}

/// 一格文本 -> 课程。缺星期或缺节次就返回 Err(原因)，由调用方收进 unparsed。
///
/// ponytail: 格子里的多个 span 之间没有分隔符，文本是黏连的（`B51016-17周第3节-第4节`），
/// 所以先定周次并记住它在原文里的位置，再**只在其后**找节次——否则会把周次范围当成节次。
fn parse_cell(seg: &str) -> Result<Course, String> {
    let day = day_of(seg).ok_or("没找到星期")?;

    // 1) 周次 + 它在原文中的结束位置
    let (weeks, weeks_text, week_end) = seg
        .split_whitespace()
        .filter(|t| t.contains('周'))
        .find_map(|t| {
            weeks_of(t).map(|(w, shown, _)| {
                let end = seg.find(t).map(|i| i + t.len()).unwrap_or(0);
                (w, shown, end)
            })
        })
        .ok_or("没找到周次")?;

    // 2) 节次：只在周次之后找，避免「3-18周」里的 3-18 被当节次
    let (start, end) = period_range_after(seg, week_end)
        .or_else(|| {
            seg.split_whitespace()
                .filter(|t| t.contains('节'))
                .find_map(|t| period_range_after(t, 0))
        })
        .or_else(|| {
            seg.split_whitespace().filter(|t| t.contains('节')).find_map(|t| {
                single_period_re().captures(t).and_then(|c| c.get(1)?.as_str().parse::<u32>().ok()).map(|n| (n, n))
            })
        })
        .ok_or("没找到节次")?;
    let (start, end) = (start.min(end), start.max(end));

    let teacher = label_value(seg, &["上课教师", "教师", "老师"]).unwrap_or_default();
    let room = label_value(seg, &["上课地点", "教室", "地点", "教学班"]).unwrap_or_default();

    let body = strip_known(seg);
    let teacher = if teacher.is_empty() { first_teacher(&body) } else { teacher };
    let name = body
        .split_whitespace()
        .flat_map(|t| t.split(|c: char| c == '[' || c == ']'))
        .map(|t| t.trim().to_string())
        .find(|t| {
            t.chars().count() >= 2
                && !t.chars().all(|c| c.is_ascii_digit() || "-—~,，、()（）[]：:".contains(c))
                && !t.contains('周')
                && !t.contains('节')
                && !t.contains('教')
                && t != &teacher
        })
        .or_else(|| {
            body.split_whitespace()
                .find(|t| !t.is_empty() && !t.contains('周') && !t.contains('节'))
                .map(|t| t.to_string())
        })
        .ok_or("没找到课程名")?;

    Ok(Course {
        name,
        teacher,
        room,
        day,
        start,
        end,
        weeks,
        weeks_text,
        color: 0,
        note: None,
        confidence: None,
    })
}

/// 取标签后面的值：按「空白 / 括号 / 逗号」断词，字符边界安全
fn value_after(s: &str, i: usize, key: &str) -> String {
    let start = i + key.len();
    let rest = &s[start..];
    let lead = rest.len() - rest.trim_start_matches([':', '：', ' ']).len();
    let val: String = rest[lead..]
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '(' && *c != '（' && *c != ',' && *c != '，')
        .collect();
    val
}

/// 「教师：张三」「教室：B510」这类标签取值
fn label_value(s: &str, keys: &[&str]) -> Option<String> {
    for k in keys {
        for (i, _) in s.match_indices(k) {
            let val = value_after(s, i, k);
            if !val.is_empty() && val != "无" && val != "暂无" {
                return Some(val);
            }
        }
    }
    None
}

/// 去掉节次/周次/教师/教室那些片段，剩下的当课程名候选
fn strip_known(seg: &str) -> String {
    let mut s = seg.to_string();
    for key in ["上课教师", "上课地点", "教师", "老师", "教室", "地点", "教学班"] {
        while let Some(i) = s.find(key) {
            let start = i + key.len();
            let vlen = value_after(&s, i, key).len();
            s.replace_range(i..start + vlen, " ");
        }
    }
    s
}

/// 从剩下的片段里猜教师名：连续 2-4 个汉字、不含课程常见字
fn first_teacher(s: &str) -> String {
    for tok in s.split_whitespace() {
        let t: String = tok.chars().filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c)).collect();
        if (2..=4).contains(&t.chars().count())
            && !["大学", "高等", "基础", "理论", "导论", "体育", "英语", "数学", "物理"].iter().any(|k| t.contains(k))
        {
            return t;
        }
    }
    String::new()
}

/// 最小 HTML 分词：把正文切成 (是否闭合标签, 标签名, class, 该标签之后、下一个标签之前的文本)。
/// 只做配对统计，目的就是**准确切出边界**——正则数嵌套 div 数不准，已经吃过一次亏。
fn tokenize(html: &str) -> Vec<(bool, String, String, String)> {
    let tag_re = regex::Regex::new(r"(?is)<\s*(/?)\s*([a-zA-Z][a-zA-Z0-9]*)\b([^>]*)>").unwrap();
    let class_re = regex::Regex::new(r#"(?is)class\s*=\s*["']([^"']*)["']"#).unwrap();
    // 文本节点：去掉可能残留的标签，解实体、压空白——注意不要用它处理整块 HTML（会把分隔空格吃掉）
    let clean = |s: &str| -> String {
        let t = if s.contains('<') { text_of(s) } else { s.to_string() };
        t.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    let mut out: Vec<(bool, String, String, String)> = Vec::new();
    let mut last = 0usize;
    for m in tag_re.find_iter(html) {
        let text = clean(&html[last..m.start()]);
        last = m.end();
        let caps = tag_re.captures(m.as_str()).unwrap();
        let closing = caps.get(1).map(|c| c.as_str() == "/").unwrap_or(false);
        let tag = caps.get(2).map(|c| c.as_str().to_ascii_lowercase()).unwrap_or_default();
        let attrs = caps.get(3).map(|c| c.as_str()).unwrap_or("");
        let class = class_re
            .captures(attrs)
            .and_then(|c| c.get(1))
            .map(|c| c.as_str().to_string())
            .unwrap_or_default();
        if !text.is_empty() {
            out.push((false, String::new(), String::new(), text));
        }
        if !closing && (tag == "br" || tag == "img" || tag == "input") {
            continue; // 自闭合标签，不参与配对
        }
        out.push((closing, tag, class, String::new()));
    }
    let tail = clean(&html[last..]);
    if !tail.is_empty() {
        out.push((false, String::new(), String::new(), tail));
    }
    out
}

/// 取文本节点（跳过标签）
fn text_nodes(html: &str) -> Vec<String> {
    tokenize(html)
        .into_iter()
        .filter(|(_, tag, _, _)| tag.is_empty())
        .map(|(_, _, _, text)| text)
        .collect()
}

/// 一行：节次标签 + 若干 (列号, 课块文本)
type GridRow = (String, Vec<(usize, String)>);

/// 这一格是不是「第N-M节」这种节次轴标签。
/// 只认**开头**：合并出来的格子里可能在中间出现别的节次字样（`第5节第3节-第4节`），
/// 用 `is_match` 会从中间命中，把上一行的课错挂到下一行。
fn is_period_label(t: &str) -> bool {
    t.chars().count() <= 12
        && !t.contains('周')
        && !t.contains('教')
        && !t.contains('：')
        && period_label_re().find(t).map(|m| m.start() == 0).unwrap_or(false)
}

/// 一格在课表里的角色
#[derive(PartialEq, Clone, Copy, Debug)]
enum CellKind {
    DayHeader, // 「星期一」这种列头
    Axis,      // 「第1-2节」这种节次轴标签
    Lesson,    // 有课
    Empty,     // 其它（表头文字、空 div、装饰）
}

#[derive(Clone, Debug)]
struct RawCell {
    text: String,
    kind: CellKind,
}

/// 一格文本 -> 角色
fn classify(t: &str) -> CellKind {
    if is_period_label(t) {
        return CellKind::Axis;
    }
    if day_of(t).is_some() && t.chars().count() <= 6 {
        return CellKind::DayHeader;
    }
    // 「第5节」这种没有 range 的轴标签也要认出来，否则整行不会断开（同样只认开头）
    if t.chars().count() <= 12
        && !t.contains('周')
        && period_label_re().find(t).map(|m| m.start() == 0).unwrap_or(false)
    {
        return CellKind::Axis;
    }
    if t.contains('周') && t.chars().count() >= 4 {
        return CellKind::Lesson;
    }
    CellKind::Empty
}

/// 从 token 流里按「一个格子 = 一个 cell/con/列头 容器」取出所有格子（含空 div，用于保持列号）
fn cells(html: &str) -> Vec<RawCell> {
    let toks = tokenize(html);
    let mut out: Vec<RawCell> = Vec::new();
    let mut depth = 0usize;
    let mut buf = String::new();
    let mut cur_is_cell = false; // 当前这一格的开标签是否带 cell/con
    for (closing, tag, class, text) in toks {
        if tag.is_empty() {
            if depth > 0 && !text.is_empty() {
                if !buf.is_empty() {
                    buf.push(' ');
                }
                buf.push_str(&text);
            }
            continue;
        }
        let is_cell = tag == "div" && class.split_whitespace().any(|c| c.contains("cell") || c.contains("con"));
        // 开标签：class 带 cell/con，或列头项（head_item/week_item），才算候选格子。
        // 注意「第5节」这种轴的文本在开标签**之后**才出现，所以不能只看开标签时的 text，
        // 要在闭标签时按累积文本再判一次（见下面的 axis_or_day）。
        let lc = class.to_ascii_lowercase();
        let starts_cell = !closing
            && tag == "div"
            && (is_cell
                || lc.contains("head_item")
                || lc.contains("week_item")
                || lc.split_whitespace().any(|c| c == "head" || c == "week" || c == "title"));
        if starts_cell {
            if depth > 0 {
                depth += 1; // 嵌套的 con 归到同一个格子
            } else {
                depth = 1;
                buf.clear();
                cur_is_cell = is_cell;
            }
            continue;
        }
        // 闭标签：**任何** </div> 都要配对，因为闭合标签没有 class
        if closing && tag == "div" && depth > 0 {
            depth -= 1;
            if depth > 0 {
                continue;
            }
            let t = buf.split_whitespace().collect::<Vec<_>>().join(" ");
            let kind = classify(&t);
            // 结算条件：
            // - 开标签带 cell/con 的：**空格子也算一格**（timetable_cell 的空位必须占一列，否则列号漂移）
            // - 其它裸 div（列头项）：只有文本像「课程 / 星期 / 节次」才算，容器那种吞多格的直接丢
            let looks_content = kind != CellKind::Empty
                || t.contains('周')
                || t.contains("教师")
                || t.contains("地点")
                || t.contains('节');
            if !cur_is_cell && (t.is_empty() || !looks_content) {
                buf.clear();
                continue;
            }
            out.push(RawCell { text: t, kind });
            buf.clear();
            continue;
        }
        // 非 cell 的 div 只是容器，不参与
    }
    out
}

/// 走一遍格子流：按节次轴切行，按累计列号定位星期。
/// 表头识别不到时按「数据行里最多 7 个课格」等分，并把这条判定写进诊断。
fn schedule_rows(html: &str) -> (Vec<u32>, Vec<GridRow>, Vec<String>) {
    let all = cells(html);
    let mut notes: Vec<String> = Vec::new();

    // 1) 列头：连续的 DayHeader 视为一行表头
    let mut header: Vec<u32> = Vec::new();
    for c in all.iter().filter(|c| c.kind == CellKind::DayHeader) {
        if let Some(d) = day_of(&c.text) {
            if !header.contains(&d) {
                header.push(d);
            }
        }
    }
    notes.push(format!("表头识别到 {} 个星期：{:?}", header.len(), header));

    // 2) 表头认不全（不是 7 天）就按数据行的课格宽度等分 7 列
    if header.len() != 7 {
        if !header.is_empty() {
            notes.push(format!("表头只有 {} 天，改用等分 7 列兜底", header.len()));
        } else {
            notes.push("没识别到表头，用等分 7 列兜底".into());
        }
        header = (1..=7).collect();
    }

    // 3) 切行 + 列号记账
    let mut rows: Vec<GridRow> = Vec::new();
    let mut cur: Option<GridRow> = None; // (节次标签, [(列号, 文本)])
    let mut col = 0usize;
    for c in all {
        match c.kind {
            CellKind::Axis => {
                if let Some(row) = cur.take() {
                    if !row.1.is_empty() {
                        rows.push(row);
                    }
                }
                cur = Some((c.text, Vec::new()));
                col = 0;
            }
            CellKind::DayHeader => {
                // 表头行里的星期不进数据行
                col += 1;
            }
            CellKind::Lesson => {
                if let Some(row) = cur.as_mut() {
                    row.1.push((col, c.text));
                }
                col += 1;
            }
            CellKind::Empty => col += 1,
        }
    }
    if let Some(row) = cur.take() {
        if !row.1.is_empty() {
            rows.push(row);
        }
    }

    // 4) 行内课格超过 7 个说明列号记账没对齐，等分修正
    let max_cells = rows.iter().map(|r| r.1.len()).max().unwrap_or(0);
    if max_cells > 7 {
        notes.push(format!("某行有 {max_cells} 个课格（>7），列号按顺序等分修正"));
        for (_, cells) in rows.iter_mut() {
            let n = cells.len();
            for (i, cell) in cells.iter_mut().enumerate() {
                cell.0 = if n <= 1 { 0 } else { i * 7 / n };
            }
        }
    }
    notes.push(format!("数据行 {} 行，单行最多 {} 个课格", rows.len(), max_cells));
    (header, rows, notes)
}
/// 金智教育 jwapp（wdkb 学生课程表）的课格字段。
/// 真实结构（某高校金智 jwapp 课表页实测，域名与账号信息已抹掉）：
/// ```html
/// <td data-role="item" data-week="1" data-begin-unit="1" data-end-unit="2" rowspan="2">
///   <div class="mtt_arrange_item" style="background-color:#FFF0CC">
///     <div class="mtt_item_kcmc">程序设计基础[02]
///       <div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc">王建国,李海涛</div></div>
///       <div class="mtt_item_room">3-18周,星期1,第1节-第2节,B510</div>
///       <div class="mtt_item_class" title="…">…</div>
///     </div>
///   </div>
/// </td>
/// ```
/// 「教师」嵌在「课程名」里面，所以取 .mtt_item_kcmc 的文本时会带出教师，
/// 必须先用 .mtt_item_jxbmc 把教师切掉。
fn kcmc_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r#"(?is)<div\b[^>]*class="[^"]*mtt_item_kcmc[^"]*"[^>]*>(.*?)<div\b[^>]*class="[^"]*(?:mtt_item_jxbmc|mtt_item_room|mtt_item_class)"#).unwrap())
}

fn jxbmc_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r#"(?is)class="[^"]*mtt_item_jxbmc[^"]*"[^>]*>(.*?)</div>"#).unwrap())
}

fn room_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r#"(?is)class="[^"]*mtt_item_room[^"]*"[^>]*>(.*?)</div>"#).unwrap())
}

/// 从课格里取出 (课程名, 教师, 周次节次教室文本串)
fn parse_jwapp_cell(cell_html: &str) -> Option<(String, String, String)> {
    let text = |re: &regex::Regex| -> String {
        re.captures(cell_html)
            .and_then(|c| c.get(1))
            .map(|m| text_of(m.as_str()))
            .unwrap_or_default()
    };
    let name = text(kcmc_re());
    let teacher = text(jxbmc_re());
    let bundle = text(room_re());
    // 课程名兜底：万一 kcmc 正则没命中，就用整格文本减去教师与 bundle
    let name = if name.is_empty() {
        text_of(cell_html)
            .replace(&bundle, "")
            .replace(&teacher, "")
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    } else {
        name
    };
    if name.is_empty() {
        return None;
    }
    Some((name, teacher, bundle))
}

/// 去重 key：同一天、同节次、同课名、同周次算同一门
fn course_key(c: &Course) -> String {
    format!("{}|{}|{}|{}|{}", c.day, c.start, c.end, c.name, c.weeks_text)
}

/// 「3-18周,星期1,第1节-第2节,B510」-> 课程
fn parse_bundle(name: &str, teacher: &str, bundle: &str) -> Result<Course, String> {
    let parts: Vec<&str> = bundle.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    let weeks_part = parts.iter().find(|p| p.contains('周')).copied().unwrap_or("");
    if weeks_part.is_empty() {
        return Err("bundle 里没有周次".into());
    }
    let day = day_of(bundle).ok_or("bundle 里没有星期")?;
    let (start, end) = period_range_after(bundle, 0).ok_or("bundle 里没有节次")?;
    let (weeks, weeks_text) = weeks_of(weeks_part).map(|(w, t, _)| (w, t)).ok_or("周次解析失败")?;
    // 教室：取既不是周次、也不是星期、也不是节次的那一段
    let room = parts
        .iter()
        .find(|p| !p.contains('周') && !p.contains("星期") && !p.contains('节'))
        .map(|s| s.to_string())
        .unwrap_or_default();
    Ok(Course {
        name: name.to_string(),
        teacher: teacher.to_string(),
        room,
        day,
        start,
        end,
        weeks,
        weeks_text,
        color: 0,
        note: None,
        confidence: None,
    })
}

/// 金智 jwapp 路径：直接按课格字段取，星期/节次/周次都在格内，不依赖列位置。
/// 返回 None 表示这不是 jwapp 课表页（交给正方路径）。
fn parse_jwapp(html: &str, debug: &mut Vec<String>) -> Option<Vec<Course>> {
    if !html.contains("mtt_item_kcmc") && !html.contains("wut_table") {
        return None;
    }
    let td_re = regex::Regex::new(r#"(?is)<td\b[^>]*>(.*?)</td>"#).unwrap();
    let mut courses: Vec<Course> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut cells = 0usize;
    let mut skipped = 0usize;
    for cap in td_re.captures_iter(html) {
        let body = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        if !body.contains("mtt_item_kcmc") {
            continue;
        }
        cells += 1;
        let Some((name, teacher, bundle)) = parse_jwapp_cell(body) else {
            skipped += 1;
            continue;
        };
        match parse_bundle(&name, &teacher, &bundle) {
            Ok(c) => {
                let key = course_key(&c);
                if !seen.contains(&key) {
                    seen.push(key);
                    courses.push(c);
                }
            }
            Err(why) => {
                skipped += 1;
                debug.push(format!("jwapp 课格解析失败（{why}）：{}", digest(&format!("{name} {bundle}"))));
            }
        }
    }
    debug.push(format!("jwapp 路径：课格 {cells} 个，跳过 {skipped} 个"));
    if courses.is_empty() {
        return None;
    }
    Some(courses)
}

/// 解析入口：先按金智 jwapp 结构化取，再退回正方/通用路径。
pub fn parse_schedule(html: &str) -> ParseScheduleResult {
    let mut debug: Vec<String> = Vec::new();

    if let Some(mut courses) = parse_jwapp(html, &mut debug) {
        // 当前周高亮（highLight / 有背景色）只影响展示，不影响导入，这里不参与
        courses.sort_by_key(|c| (c.day, c.start));
        for (i, c) in courses.iter_mut().enumerate() {
            c.color = (i % 8) as u32;
        }
        debug.push(format!("识别为金智 jwapp 课表，共 {} 门", courses.len()));
        return ParseScheduleResult { courses, unparsed: Vec::new(), debug };
    }
    debug.push("不是金智 jwapp 结构，走通用/正方路径".into());

    let mut courses: Vec<Course> = Vec::new();
    let mut unparsed: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    let (header, rows, row_notes) = schedule_rows(html);
    debug.extend(row_notes);

    // 把行的节次标签拼进每个课块，让 parse_cell 有权威节次可用
    let mut blocks: Vec<(Option<u32>, String)> = Vec::new();
    for (label, cells) in &rows {
        for (i, text) in cells {
            if text.len() < 4 || !text.contains('周') {
                continue;
            }
            let day = day_of(text).or_else(|| header.get(*i).copied());
            blocks.push((day, format!("{text} {label}")));
        }
    }
    debug.push(format!("按行取到候选课块 {} 个", blocks.len()));
    if blocks.is_empty() {
        // 纯文本 / 单格场景
        blocks = text_nodes(html)
            .into_iter()
            .filter(|t| t.len() >= 4 && t.contains('周'))
            .map(|t| (day_of(&t).or_else(|| header.first().copied()), t))
            .collect();
        debug.push(format!("退回文本节点候选 {} 个", blocks.len()));
    }

    for (i, (day, block)) in blocks.iter().enumerate() {
        let Some(day) = day else {
            debug.push(format!("第{i}块推不出星期：{}", digest(block)));
            continue;
        };
        let name = DAY_NAMES.get(*day as usize).copied().unwrap_or("");
        let seg = format!("{block} {name}");
        match parse_cell(&seg) {
            Ok(c) => {
                let key = course_key(&c);
                if !seen.contains(&key) {
                    seen.push(key);
                    courses.push(c);
                }
            }
            Err(why) => {
                debug.push(format!("第{i}块解析失败（{why}）：{}", digest(block)));
                unparsed.push(format!("{why}：{block}"));
            }
        }
    }

    for (i, c) in courses.iter_mut().enumerate() {
        c.color = (i % 8) as u32;
    }

    if courses.is_empty() && unparsed.is_empty() {
        unparsed.push("没在 HTML 里找到课表格子：确认已登录、并停留在「我的课表」页面，而不是登录页或首页。".into());
    }
    ParseScheduleResult { courses, unparsed, debug }
}

/// 诊断用摘要：一行内看得到前后内容，不至于刷屏
fn digest(s: &str) -> String {
    let t: String = s.chars().take(48).collect();
    format!("\"{t}\"({}字)", s.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实抓包夹具（金智 jwapp 课表页，2026-09-22 实测；教师/班级/域名已替换为虚构值）。
    /// 结构：`<table class="wut_table">`，8 列 = 节次轴 + 星期一…星期日；
    /// 课格 `<td data-role="item" data-week=N data-begin-unit data-end-unit rowspan>`，
    /// 内含 `.mtt_item_kcmc`（课程名）/`.mtt_item_jxbmc`（教师）/`.mtt_item_room`（`3-18周,星期1,第1节-第2节,B510`）。
    /// 已脱敏：删掉学号、姓名、班级等信息，只保留结构与课程字段。
    const JWAPP_HTML: &str = r#"
<html><body>
<table class="wut_table" onselectstart="return false">
  <tr>
    <td class="mtt_bgcolor_grey" style="width:13%">节次/星期</td>
    <td class="mtt_bgcolor_grey" style="width:13%" data-week="1">星期一</td>
    <td class="mtt_bgcolor_grey highLight" style="width:13%" data-week="2">星期二</td>
    <td class="mtt_bgcolor_grey" style="width:13%" data-week="3">星期三</td>
    <td class="mtt_bgcolor_grey" style="width:13%" data-week="4">星期四</td>
    <td class="mtt_bgcolor_grey" style="width:13%" data-week="5">星期五</td>
    <td class="mtt_bgcolor_grey" style="width:13%" data-week="6">星期六</td>
    <td class="mtt_bgcolor_grey" style="width:13%" data-week="7">星期日</td>
  </tr>
  <tr>
    <td class="mtt_bgcolor_grey">第1节</td>
    <td data-role="item" data-week="1" data-begin-unit="1" data-end-unit="2" rowspan="2"><div class="mtt_arrange_item " style="background-color:#FFF0CC"><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">程序设计基础[02]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">王建国,李海涛</div></div><div class="mtt_item_room">3-18周,星期1,第1节-第2节,B510</div><div class="mtt_item_class" title="示例班级">示例班级</div></div></div></td>
    <td data-role="item" data-week="2" data-begin-unit="1" data-end-unit="2" rowspan="2" class="highLight"><div class="mtt_arrange_item " style="background-color:#FFF0CC"><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">大学英语 I[69]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">张丽</div></div><div class="mtt_item_room">3-18周,星期2,第1节-第2节,教学B201</div><div class="mtt_item_class" title="示例班级">示例班级</div></div></div></td>
    <td data-role="item" data-week="3" data-begin-unit="1" data-end-unit="2" rowspan="2"></td>
    <td data-role="item" data-week="4" data-begin-unit="1" data-end-unit="2" rowspan="2"><div class="mtt_arrange_item " style="background-color:#FFF0CC"><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">思想道德与法治[18]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">陈晓明</div></div><div class="mtt_item_room">3-15周(单),星期4,第1节-第2节,教学B210</div></div></div></td>
    <td data-role="item" data-week="5" data-begin-unit="1" data-end-unit="2" rowspan="2"><div class="mtt_arrange_item " style="background-color:#FFF0CC"><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">体育Ⅰ[109]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">刘伟,赵强</div></div><div class="mtt_item_room">4-18周,星期5,第1节-第2节,操场</div></div></div></td>
    <td data-role="item" data-week="6" data-begin-unit="1" data-end-unit="2" rowspan="2"></td>
    <td data-role="item" data-week="7" data-begin-unit="1" data-end-unit="2" rowspan="2"></td>
  </tr>
  <tr><td class="mtt_bgcolor_grey">第2节</td></tr>
  <tr>
    <td class="mtt_bgcolor_grey">第3节</td>
    <td data-role="item" data-week="1" data-begin-unit="3" data-end-unit="4" rowspan="2"><div class="mtt_arrange_item "><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">程序设计基础[02]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">王建国,李海涛</div></div><div class="mtt_item_room">16-17周,星期1,第3节-第4节,B510</div></div></div></td>
    <td data-role="item" data-week="2" data-begin-unit="3" data-end-unit="4" rowspan="2" class="highLight"><div class="mtt_arrange_item "><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">形势与政策1[33]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">孙敏</div></div><div class="mtt_item_room">11-13周(单),星期2,第3节-第4节,教学B210</div></div></div></td>
    <td data-role="item" data-week="3" data-begin-unit="3" data-end-unit="4" rowspan="2"><div class="mtt_arrange_item "><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">高等数学BⅠ[02]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">吴静</div></div><div class="mtt_item_room">3-18周,星期3,第3节-第4节,教学B301</div></div></div></td>
    <td data-role="item" data-week="4" data-begin-unit="3" data-end-unit="4" rowspan="2"><div class="mtt_arrange_item "><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">计算机导论[02]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">郑鹏,许文</div></div><div class="mtt_item_room">3-18周,星期4,第3节-第4节,教学B301</div></div></div></td>
    <td data-role="item" data-week="5" data-begin-unit="3" data-end-unit="4" rowspan="2"><div class="mtt_arrange_item "><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">高等数学BⅠ[02]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">吴静</div></div><div class="mtt_item_room">3-18周,星期5,第3节-第4节,教学B301</div></div></div></td>
    <td data-role="item" data-week="6" data-begin-unit="3" data-end-unit="4" rowspan="2"></td>
    <td data-role="item" data-week="7" data-begin-unit="3" data-end-unit="4" rowspan="2"></td>
  </tr>
  <tr><td class="mtt_bgcolor_grey">第4节</td></tr>
  <tr>
    <td class="mtt_bgcolor_grey">第5节</td>
    <td data-role="item" data-week="1" data-begin-unit="5" data-end-unit="6" rowspan="2"><div class="mtt_arrange_item "><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">思想道德与法治[18]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">陈晓明</div></div><div class="mtt_item_room">3-18周,星期1,第5节-第6节,教学B210</div></div></div></td>
    <td data-role="item" data-week="2" data-begin-unit="5" data-end-unit="6" rowspan="2"><div class="mtt_arrange_item "><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">高等数学BⅠ[02]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">吴静</div></div><div class="mtt_item_room">14-15周,星期2,第5节-第6节,教学B301</div></div></div></td>
    <td data-role="item" data-week="3" data-begin-unit="5" data-end-unit="6" rowspan="2"><div class="mtt_arrange_item "><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">计算机导论[02]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">郑鹏,许文</div></div><div class="mtt_item_room">16周,星期3,第5节-第6节,教学B310</div></div></div></td>
    <td data-role="item" data-week="4" data-begin-unit="5" data-end-unit="6" rowspan="2"><div class="mtt_arrange_item "><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">体育Ⅰ[109]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">刘伟,赵强</div></div><div class="mtt_item_room">17周,星期4,第5节-第6节,教学B201</div></div></div></td>
    <td data-role="item" data-week="5" data-begin-unit="5" data-end-unit="6" rowspan="2"></td>
    <td data-role="item" data-week="6" data-begin-unit="5" data-end-unit="6" rowspan="2"></td>
    <td data-role="item" data-week="7" data-begin-unit="5" data-end-unit="6" rowspan="2"></td>
  </tr>
  <tr><td class="mtt_bgcolor_grey">第6节</td></tr>
  <tr>
    <td class="mtt_bgcolor_grey">第7节</td>
    <td data-role="item" data-week="1" data-begin-unit="7" data-end-unit="8" rowspan="2"><div class="mtt_arrange_item "><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">军事理论[37]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">何军</div></div><div class="mtt_item_room">3-15周,星期1,第7节-第8节,第2阶梯教室</div></div></div></td>
    <td data-role="item" data-week="2" data-begin-unit="7" data-end-unit="8" rowspan="2" class="highLight"></td>
    <td data-role="item" data-week="3" data-begin-unit="7" data-end-unit="8" rowspan="2"></td>
    <td data-role="item" data-week="4" data-begin-unit="7" data-end-unit="8" rowspan="2"></td>
    <td data-role="item" data-week="5" data-begin-unit="7" data-end-unit="8" rowspan="2"><div class="mtt_arrange_item "><div class="mtt_item_tzkcicon"></div><div class="mtt_item_kcmc ">程序设计基础[02]<div class="bh-clearfix jsrs-cont"><div class="mtt_item_jxbmc bh-pull-left">王建国,李海涛</div></div><div class="mtt_item_room">3-18周,星期5,第7节-第8节,B514</div></div></div></td>
    <td data-role="item" data-week="6" data-begin-unit="7" data-end-unit="8" rowspan="2"></td>
    <td data-role="item" data-week="7" data-begin-unit="7" data-end-unit="8" rowspan="2"></td>
  </tr>
  <tr><td class="mtt_bgcolor_grey">第8节</td></tr>
  <tr>
    <td class="mtt_bgcolor_grey">第9节</td>
    <td data-role="item" data-week="1" data-begin-unit="9" data-end-unit="11" rowspan="3"></td>
    <td data-role="item" data-week="2" data-begin-unit="9" data-end-unit="11" rowspan="3" class="highLight"></td>
    <td data-role="item" data-week="3" data-begin-unit="9" data-end-unit="11" rowspan="3"></td>
    <td data-role="item" data-week="4" data-begin-unit="9" data-end-unit="11" rowspan="3"></td>
    <td data-role="item" data-week="5" data-begin-unit="9" data-end-unit="11" rowspan="3"></td>
    <td data-role="item" data-week="6" data-begin-unit="9" data-end-unit="11" rowspan="3"></td>
    <td data-role="item" data-week="7" data-begin-unit="9" data-end-unit="11" rowspan="3"></td>
  </tr>
</table>
</body></html>
"#;

    /// 金智 jwapp 真实结构：星期/节次/周次都在课格里，不靠列位置；断言每一天每一门的 day/节次/周次
    #[test]
    fn parses_jwapp_real_structure() {
        let r = parse_schedule(JWAPP_HTML);
        assert!(r.unparsed.is_empty(), "不该有未解析行：{:?}", r.unparsed);
        assert_eq!(r.courses.len(), 15, "实际：{:?}", r.courses.iter().map(|c| (c.name.clone(), c.day, c.start, c.weeks_text.clone())).collect::<Vec<_>>());

        let find = |name: &str, weeks: &str| {
            r.courses
                .iter()
                .find(|c| c.name == name && c.weeks_text == weeks)
                .unwrap_or_else(|| panic!("没解析出 {name} {weeks}；实际 {:?}", r.courses.iter().map(|c| (c.name.clone(), c.weeks_text.clone())).collect::<Vec<_>>()))
        };

        // 周一
        let a = find("程序设计基础[02]", "3-18");
        assert_eq!((a.day, a.start, a.end), (1, 1, 2));
        assert_eq!(a.teacher, "王建国,李海涛");
        assert_eq!(a.room, "B510");
        assert_eq!(find("程序设计基础[02]", "16-17").start, 3);
        assert_eq!(find("思想道德与法治[18]", "3-18").start, 5);
        let mil = find("军事理论[37]", "3-15");
        assert_eq!((mil.day, mil.start, mil.end), (1, 7, 8));
        assert_eq!(mil.room, "第2阶梯教室");

        // 周二：单周
        let odd = find("形势与政策1[33]", "11-13单");
        assert_eq!((odd.day, odd.start, odd.end), (2, 3, 4));
        assert_eq!(odd.weeks, vec![11, 13]);
        assert_eq!(find("大学英语 I[69]", "3-18").day, 2);

        // 周三：单节周次（16周）
        let intro = find("计算机导论[02]", "16");
        assert_eq!((intro.day, intro.start, intro.end), (3, 5, 6));
        assert_eq!(intro.weeks, vec![16]);

        // 周四：单周 + 单周次
        assert_eq!(find("思想道德与法治[18]", "3-15单").weeks, vec![3, 5, 7, 9, 11, 13, 15]);
        let pe = find("体育Ⅰ[109]", "17");
        assert_eq!((pe.day, pe.start, pe.end), (4, 5, 6));

        // 周五：操场 + 7-8 节的另一间教室
        let pe2 = find("体育Ⅰ[109]", "4-18");
        assert_eq!((pe2.day, pe2.start, pe2.end), (5, 1, 2));
        assert_eq!(pe2.room, "操场");
        let prog5 = r.courses.iter().find(|c| c.name == "程序设计基础[02]" && c.day == 5).unwrap();
        assert_eq!((prog5.start, prog5.end), (7, 8));
        assert_eq!(prog5.room, "B514");
    }

    /// ponytail: 手写夹具（8 列 = 节次轴 + 周一…周日），**不是真实抓包**。
    /// 覆盖并断言：连续周、单周(11-13周(单))、跨节(第3节-第4节)、跨节(第7节-第8节)、列位置定星期。
    /// 已知不支持（见下面 `known_gaps` 测试）：<br>
    /// - 节次轴单元格被 rowspan 合并、文本中间又出现别的节次字样时（`第5节第3节-第4节`），行会串位；<br>
    /// - 同一格里塞两门课（`军事理论…心理健康…`）不会拆成两门。
    /// 这两种都要一份真实 outerHTML 才能定规则，所以先不做补救（宁可少认，也不乱认）。
    const ZF_HTML: &str = r#"
<html><body>
<div class="timetable">
  <div class="timetable_head">
      <div class="timetable_head_item">节次</div>
      <div class="timetable_head_item">星期一</div>
      <div class="timetable_head_item">星期二</div>
      <div class="timetable_head_item">星期三</div>
      <div class="timetable_head_item">星期四</div>
      <div class="timetable_head_item">星期五</div>
      <div class="timetable_head_item">星期六</div>
      <div class="timetable_head_item">星期日</div>
  </div>
  <div class="timetable_body">
    <div class="timetable_row">
      <div class="timetable_cell">第1-2节</div>
      <div class="timetable_cell"><div class="timetable_con"><span class="title">程序设计基础[02]</span><span class="teacher">上课教师：张伟</span><span class="room">上课地点：教学B510</span><span class="weeks">3-18周</span><span class="period">第1节-第2节</span></div></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"><div class="timetable_con"><span class="title">大学英语I[69]</span><span class="teacher">上课教师：李娜</span><span class="room">上课地点：教学B201</span><span class="weeks">3-18周</span><span class="period">第1节-第2节</span></div></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
    </div>
    <div class="timetable_row">
      <div class="timetable_cell">第3-4节</div>
      <div class="timetable_cell"><div class="timetable_con"><span class="title">程序设计基础[02]</span><span class="teacher">上课教师：张伟</span><span class="room">上课地点：教学B510</span><span class="weeks">16-17周</span><span class="period">第3节-第4节</span></div></div>
      <div class="timetable_cell"><div class="timetable_con"><span class="title">形势与政策[33]</span><span class="teacher">上课教师：王强</span><span class="room">上课地点：教学B210</span><span class="weeks">11-13周(单)</span><span class="period">第3节-第4节</span></div></div>
      <div class="timetable_cell"><div class="timetable_con"><span class="title">计算机导论[02]</span><span class="teacher">上课教师：陈磊</span><span class="room">上课地点：教学B301</span><span class="weeks">3-18周</span><span class="period">第3节-第4节</span></div></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
    </div>
    <div class="timetable_row">
      <div class="timetable_cell">第5节</div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"><div class="timetable_con"><span class="title">体育I[109]</span><span class="teacher">上课教师：赵六</span><span class="room">上课地点：教学B201</span><span class="weeks">17周</span><span class="period">第5节</span></div></div>
      <div class="timetable_cell"><div class="timetable_con"><span class="title">军事理论[37]</span><span class="teacher">上课教师：钱七</span><span class="room">上课地点：2阶课教室</span><span class="weeks">3-15周</span><span class="period">第5节</span></div><div class="timetable_con"><span class="title">心理健康[11]</span><span class="teacher">上课教师：孙八</span><span class="room">上课地点：教学B202</span><span class="weeks">1-8周(双)</span><span class="period">第5节</span></div></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
    </div>
    <div class="timetable_row">
      <div class="timetable_cell">第7-8节</div>
      <div class="timetable_cell"><div class="timetable_con"><span class="title">高等数学B[02]</span><span class="teacher">上课教师：刘洋</span><span class="room">上课地点：教学B301</span><span class="weeks">3-18周</span><span class="period">第7节-第8节</span></div></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
      <div class="timetable_cell"></div>
    </div>
  </div>
</div>
</body></html>
"#;

    #[test]
    fn parses_weeks_range_and_parity() {
        assert_eq!(parse_weeks("3-18").0, (3..=18).collect::<Vec<u32>>());
        assert_eq!(parse_weeks("11-13单").0, vec![11, 13]);
        assert_eq!(parse_weeks("3,5,7").0, vec![3, 5, 7]);
        assert_eq!(parse_weeks("17").0, vec![17]);
        assert_eq!(parse_weeks("1-8双").0, vec![2, 4, 6, 8]);
    }

    /// ponytail: 手写夹具（8 列 = 节次轴 + 周一…周日），**不是真实抓包**。
    /// 覆盖并断言：连续周、单周(11-13周(单))、单节(第5节)、跨节(第3节-第4节 / 第7节-第8节)、
    /// 空 td 占位让列号不错位、同一天不同周次算两门课。
    /// 已知缺口见 `known_gaps_are_not_silently_wrong`。
    #[test]
    fn parses_new_zhengfang_cells() {
        let r = parse_schedule(ZF_HTML);
        assert!(r.unparsed.is_empty(), "不该有未解析行：{:?}", r.unparsed);
        assert_eq!(r.courses.len(), 8, "实际：{:?}", r.courses.iter().map(|c| (c.name.clone(), c.day, c.start, c.weeks_text.clone())).collect::<Vec<_>>());

        let find = |name: &str, weeks: &str| {
            r.courses
                .iter()
                .find(|c| c.name == name && c.weeks_text == weeks)
                .unwrap_or_else(|| panic!("没解析出 {name} {weeks}；实际：{:?}", r.courses.iter().map(|c| (c.name.clone(), c.weeks_text.clone())).collect::<Vec<_>>()))
        };

        // 连续周 + 跨节 + 周一（列 0）
        let prog = find("程序设计基础", "3-18");
        assert_eq!((prog.day, prog.start, prog.end), (1, 1, 2));
        assert_eq!(prog.weeks.len(), 16);
        assert_eq!(prog.teacher, "张伟");
        assert_eq!(prog.room, "教学B510");

        // 空 td 占位生效：列 4 = 周五
        let eng = find("大学英语I", "3-18");
        assert_eq!((eng.day, eng.start, eng.end), (5, 1, 2));

        // 同一行、不同周次 = 独立课程
        let prog2 = find("程序设计基础", "16-17");
        assert_eq!((prog2.day, prog2.start, prog2.end), (1, 3, 4));
        assert_eq!(prog2.weeks, vec![16, 17]);

        // 单周：11-13周(单) -> [11,13]，周二（列 1）
        let odd = find("形势与政策", "11-13单");
        assert_eq!((odd.day, odd.start, odd.end), (2, 3, 4));
        assert_eq!(odd.weeks, vec![11, 13]);

        // 周三（列 2）
        let intro = find("计算机导论", "3-18");
        assert_eq!((intro.day, intro.start, intro.end), (3, 3, 4));

        // 单节：第5节 -> start=end=5，周四（列 3）
        let pe = find("体育I", "17");
        assert_eq!((pe.day, pe.start, pe.end), (4, 5, 5));
        assert_eq!(pe.weeks, vec![17]);

        // 同一行另一列的课：周五（列 4）
        let mil = find("军事理论", "3-15");
        assert_eq!((mil.day, mil.start, mil.end), (5, 5, 5));
        assert_eq!(mil.room, "2阶课教室");

        // 跨节 7-8 节，周一
        let math = find("高等数学B", "3-18");
        assert_eq!((math.day, math.start, math.end), (1, 7, 8));

        assert!(r.courses.iter().all(|c| (1..=7).contains(&c.day)));
        assert!(r.courses.iter().all(|c| c.start >= 1 && c.end >= c.start));
    }

    /// 已知缺口：同一格里塞两门课（两个 `<div class="timetable_con">`）时，只会认出一门。
    /// 要拆开必须先按 con 边界切块、再逐块解析——需要一份真实页面确认这种排布真的存在，故暂不做。
    #[test]
    fn known_gaps_are_not_silently_wrong() {
        let r = parse_schedule(ZF_HTML);
        assert!(!r.courses.iter().any(|c| c.name == "心理健康"), "同格多课当前不拆，出现说明行为变了");
        // 但同格里第一门课是被认出来的（军事理论），说明不是整格丢失
        assert!(r.courses.iter().any(|c| c.name == "军事理论"));
    }

    #[test]
    fn handles_bare_text_cells() {
        let html = r#"<table><tr><td>程序设计基础 张伟 B510 周一 1-2节 3-18周</td></tr></table>"#;
        let r = parse_schedule(html);
        assert_eq!(r.courses.len(), 1, "{:?}", r.unparsed);
        let c = &r.courses[0];
        assert_eq!((c.day, c.start, c.end), (1, 1, 2));
        assert_eq!(c.weeks.len(), 16);
    }

    #[test]
    fn reports_unparsed_lines_with_reason() {
        // 一格缺节次（应进 unparsed），一格完整
        let html = r#"<div class="timetable_cell">周一 高等数学</div><div class="timetable_cell">大学英语 周二 1-2节 3-18周</div>"#;
        let r = parse_schedule(html);
        assert_eq!(r.courses.len(), 1, "只有缺节次的那格才该失败：{:?} / {:?}", r.unparsed, r.debug);
        assert_eq!(r.unparsed.len(), 1);
        assert!(r.unparsed[0].contains("没找到周次") || r.unparsed[0].contains("没找到节次"), "{}", r.unparsed[0]);
    }

    #[test]
    fn gives_friendly_hint_when_no_table() {
        let r = parse_schedule("<html><body><div id=\"app\">登录</div></body></html>");
        assert!(r.courses.is_empty());
        assert_eq!(r.unparsed.len(), 1);
        assert!(r.unparsed[0].contains("我的课表"));
    }
}
