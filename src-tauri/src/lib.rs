use serde::{Deserialize, Serialize};
use std::time::Duration;

mod admin;

/// Umi-OCR 默认地址（可在 Umi-OCR 全局设置里改端口）
const UMI_OCR_URL: &str = "http://127.0.0.1:1224/api/ocr";
const OCR_TIMEOUT_SECS: u64 = 30;

/// 与前端 src/lib/ocr.ts 的 LOW_CONFIDENCE 保持一致
#[allow(dead_code)]
const LOW_CONFIDENCE: f64 = 0.7;

/// 教务导入窗口的 label（capabilities 里按这个授权事件）
const ADMIN_LABEL: &str = "admin";

/// 注入到教务窗口的「解析导入」按钮脚本。
/// 用 eval 注入而不是打包第二个 HTML 页面：少一个 Vite 多页入口、少一份产物。
/// 抓的是 documentElement.outerHTML（新版正方课表常渲染在 iframe 里，iframe 同源时 html() 已包含其内容；
/// 不同源则由用户手动粘贴兜底）。
const ADMIN_BUTTON_JS: &str = r#"
(function () {
  if (document.getElementById('__timetable_parse_btn')) return;
  var b = document.createElement('button');
  b.id = '__timetable_parse_btn';
  b.textContent = '解析导入到课表';
  b.style.cssText = 'position:fixed;right:18px;bottom:18px;z-index:2147483647;padding:10px 16px;border:0;border-radius:8px;background:#2563eb;color:#fff;font:14px/1.2 system-ui;cursor:pointer;box-shadow:0 4px 14px rgba(0,0,0,.25)';
  b.onclick = function () {
    b.disabled = true;
    b.textContent = '解析中…';
    window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {
      event: 'admin-html',
      payload: { url: location.href, html: document.documentElement.outerHTML }
    }).then(function () {
      b.textContent = '已发送，回课表应用看预览';
    }).catch(function (e) {
      b.disabled = false;
      b.textContent = '解析导入到课表';
      alert('没能把页面发给课表应用：' + e);
    });
  };
  document.body.appendChild(b);
})();
"#;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    pub name: String,
    pub teacher: String,
    pub room: String,
    pub day: u32,
    pub start: u32,
    pub end: u32,
    pub weeks: Vec<u32>,
    pub weeks_text: String,
    pub color: u32,
    /// 备注（与前端 Course.note 对齐；教务导入目前不用，OCR 也不用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

/// 识别结果：能解析成课程的 + 需要人工确认的未解析行。
/// 前端 src/lib/ocr.ts 的 OcrResult 是同一形状（字段名驼峰）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    pub courses: Vec<Course>,
    pub unparsed: Vec<OcrUnparsed>,
}

#[derive(Serialize)]
pub struct OcrUnparsed {
    pub text: String,
    pub confidence: f64,
}

/// Umi-OCR /api/ocr 的返回：`data` 在成功时是文本块数组，失败时是错误字符串。
#[derive(Deserialize)]
struct OcrResponse {
    code: i64,
    data: serde_json::Value,
}

#[derive(Deserialize)]
struct OcrLine {
    text: String,
    #[serde(default)]
    score: Option<f64>,
}

#[tauri::command]
async fn http_get(url: String) -> Result<String, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("只支持 http/https".into());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

/// 桌面端本地 OCR：图片只在内存里转成 base64，发给 127.0.0.1 上的 Umi-OCR，不出机器。
#[tauri::command]
async fn ocr_image(path: String) -> Result<OcrResult, String> {
    use base64::Engine;
    let bytes = std::fs::read(&path).map_err(|e| format!("读不到图片：{e}"))?;
    let base64 = base64::engine::general_purpose::STANDARD.encode(bytes);

    // ocr.cls: Umi-OCR 3.x 的 PaddleOCR 插件要求显式传布尔值，2.x 会忽略未知参数。
    let body = serde_json::json!({
        "base64": base64,
        "options": { "ocr.cls": false, "data.format": "dict" }
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(OCR_TIMEOUT_SECS))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(UMI_OCR_URL)
        .json(&body)
        .send()
        .await
        .map_err(|_| {
            "未检测到本地 OCR 服务，请启动 Umi-OCR 或改用 Web 端上传识别（需要先在设置里打开允许上传）".to_string()
        })?;

    if !resp.status().is_success() {
        return Err(format!("本地 OCR 服务返回 HTTP {}", resp.status()));
    }
    let text = resp.text().await.map_err(|e| e.to_string())?;
    let parsed: OcrResponse = serde_json::from_str(&text).map_err(|e| format!("OCR 返回格式不认识：{e}"))?;

    if parsed.code != 100 {
        return Err(match &parsed.data {
            serde_json::Value::String(s) => format!("OCR 失败（code {}）：{s}", parsed.code),
            _ => format!("OCR 失败（code {}）", parsed.code),
        });
    }

    let lines: Vec<OcrLine> = serde_json::from_value(parsed.data).map_err(|e| format!("OCR 结果解析失败：{e}"))?;
    Ok(parse_lines(&lines))
}

/// 注意用 chars().position()：str::find 返回的是字节偏移，中文会算错
fn cjk_num(c: char) -> Option<u32> {
    "一二三四五六日天".chars().position(|x| x == c).map(|i| if i == 6 { 7 } else { i as u32 + 1 })
}

/// "1-8单" / "1-16" / "1,3,5" -> 周次
fn parse_weeks(s: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for part in s.split([',', '，']) {
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
            .split(['-', '~'])
            .filter_map(|x| x.trim().parse::<u32>().ok())
            .collect();
        match nums.as_slice() {
            [a, b] => {
                for w in (*a).min(*b)..=(*a).max(*b) {
                    if w >= 1 && w <= 30 && (parity < 0 || w % 2 == parity as u32) {
                        out.push(w);
                    }
                }
            }
            [a] => out.push(*a),
            _ => {}
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// 一行 OCR 文本 -> 一门课。拿不到星期或节次就跳过：宁可少认，也不往用户课表里灌垃圾。
fn parse_line(text: &str, score: Option<f64>) -> Option<Course> {
    let toks: Vec<&str> = text.split_whitespace().collect();

    // 星期：取第一个能读出「周x / 星期x」的 token，并记住它的下标
    // （不能用 contains("周") 一刀切排除，否则 "1-16周" 这种周次 token 也会被排除）
    let mut day = None;
    let mut day_tok = usize::MAX;
    'outer: for (i, tok) in toks.iter().enumerate() {
        for key in ["周", "星期", "礼拜"] {
            if let Some(pos) = tok.find(key) {
                if let Some(c) = tok[pos + key.len()..].chars().next() {
                    let d = match c {
                        '1'..='7' => c.to_digit(10),
                        _ => cjk_num(c),
                    };
                    if let Some(d) = d {
                        day = Some(d);
                        day_tok = i;
                        break 'outer;
                    }
                }
            }
        }
    }
    // 节次：取 "节" 前面那段里的数字
    let mut span = None;
    for (i, _) in text.match_indices('节') {
        let token: String = text[..i]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit() || "-~第 ".contains(*c))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let nums: Vec<u32> = token
            .trim()
            .trim_start_matches('第')
            .split(['-', '~'])
            .filter_map(|x| x.trim().parse::<u32>().ok())
            .collect();
        if !nums.is_empty() {
            span = Some((nums[0], *nums.last().unwrap()));
            break;
        }
    }
    let (day, (start, end)) = (day?, span?);
    let (start, end) = (start.min(end), start.max(end));
    if start == 0 {
        return None;
    }

    let mut used = vec![false; toks.len()];

    let room = toks
        .iter()
        .position(|t| t.contains(['教', '楼', '室', '馆', '区']))
        .map(|i| {
            used[i] = true;
            toks[i].to_string()
        })
        .unwrap_or_default();

    // 周次 token：1-8单 / 1,3,5 / 1-16周 —— 整块只有数字和分隔符，避免把“3-4节”当周次
    let mut weeks_text = "1-16".to_string();
    let mut weeks: Vec<u32> = Vec::new();
    if let Some(i) = toks.iter().enumerate().position(|(i, t)| {
        i != day_tok
            && !used[i]
            && !t.is_empty()
            && t.chars().all(|c| c.is_ascii_digit() || ",，、-~周单双".contains(c))
            && t.chars().any(|c| c.is_ascii_digit())
    }) {
        used[i] = true;
        let stripped = toks[i].trim_end_matches('周').to_string();
        let parsed = parse_weeks(&stripped);
        if !parsed.is_empty() {
            weeks_text = stripped;
            // 注意这里是外层的 weeks：早先写成局部 let weeks 导致 inferred_weeks 误判、多扣 0.15
            weeks = parsed;
        }
    }

    // 教师名按 ≤3 字判断：4 字的“高等数学”是课名，不是老师
    let teacher = toks
        .iter()
        .enumerate()
        .find(|(i, t)| {
            !used[*i]
                && t.chars().count() <= 3
                && t.chars().all(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
        })
        .map(|(i, t)| {
            used[i] = true;
            t.to_string()
        })
        .unwrap_or_default();

    let name = toks
        .iter()
        .enumerate()
        .find(|(i, t)| !used[*i] && t.chars().count() > 1)
        .map(|(_, t)| t.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| text.to_string());

    // 没有 “N周” 字样时，周次是默认的 1-16，不算可靠
    let inferred_weeks = weeks.is_empty();
    if inferred_weeks {
        weeks = (1..=16).collect();
    }
    let mut confidence = score.unwrap_or(0.6);
    if room.is_empty() {
        confidence -= 0.1;
    }
    if inferred_weeks {
        confidence -= 0.15;
    }
    if teacher.is_empty() {
        confidence -= 0.05;
    }

    Some(Course {
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
        confidence: Some(confidence.clamp(0.0, 1.0)),
    })
}

fn parse_lines(lines: &[OcrLine]) -> OcrResult {
    let mut courses: Vec<Course> = Vec::new();
    let mut unparsed: Vec<OcrUnparsed> = Vec::new();
    for line in lines {
        let text = line.text.trim();
        if text.is_empty() {
            continue;
        }
        match parse_line(text, line.score) {
            Some(mut c) => {
                c.color = (courses.len() % 8) as u32;
                courses.push(c);
            }
            // 解析不出来的行也带回去，前端归入「需人工确认」并显示原文
            None => unparsed.push(OcrUnparsed {
                text: text.to_string(),
                confidence: 0.3,
            }),
        }
    }
    OcrResult { courses, unparsed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_normal_line() {
        let c = parse_line("高等数学 张伟 一教302 周一 1-2节 1-16周", Some(0.97)).unwrap();
        assert_eq!(c.name, "高等数学");
        assert_eq!(c.teacher, "张伟");
        assert_eq!(c.room, "一教302");
        assert_eq!((c.day, c.start, c.end), (1, 1, 2));
        assert_eq!(c.weeks_text, "1-16");
        assert_eq!(c.weeks.len(), 16);
        assert!(c.confidence.unwrap() > 0.8);
    }

    #[test]
    fn parses_odd_weeks_and_full_width() {
        let c = parse_line("英语 李娜 外语楼205 星期三 3-4节 1-8单", None).unwrap();
        assert_eq!((c.day, c.start, c.end), (3, 3, 4));
        assert_eq!(c.weeks, vec![1, 3, 5, 7]);
    }

    #[test]
    fn skips_lines_without_time() {
        assert!(parse_line("2025-2026 学年第一学期课表", Some(0.99)).is_none());
        assert!(parse_line("周一 高数", Some(0.99)).is_none());
        assert!(parse_line("", Some(0.99)).is_none());
    }

    #[test]
    fn missing_weeks_lowers_confidence() {
        let c = parse_line("数据结构 王强 计算机楼401 周五 5-6节", Some(0.95)).unwrap();
        assert_eq!(c.weeks_text, "1-16");
        assert!(c.confidence.unwrap() < 0.9);
    }

    #[test]
    fn parse_weeks_matches_frontend() {
        assert_eq!(parse_weeks("1-8双"), vec![2, 4, 6, 8]);
        assert_eq!(parse_weeks("1,3,5"), vec![1, 3, 5]);
        assert_eq!(parse_weeks("垃圾"), Vec::<u32>::new());
    }

    /// 与前端 test/ocr.test.mjs 的阈值边界用例一一对应
    #[test]
    fn low_confidence_threshold_boundary() {
        let line = "高等数学 张伟 一教302 周一 1-2节 1-16周";
        for (score, low) in [(0.69, true), (0.70, false), (0.71, false)] {
            let c = parse_line(line, Some(score)).unwrap();
            assert_eq!(c.confidence.unwrap(), score);
            assert_eq!(c.confidence.unwrap() < LOW_CONFIDENCE, low, "score={score}");
        }
    }

    #[test]
    fn unparsed_lines_are_kept_with_raw_text() {
        let lines = vec![
            OcrLine { text: "2025-2026 学年第一学期课表".into(), score: Some(0.99) },
            OcrLine { text: "高等数学 张伟 一教302 周一 1-2节 1-16周".into(), score: Some(0.97) },
            OcrLine { text: "周一 高数".into(), score: Some(0.5) },
        ];
        let r = parse_lines(&lines);
        assert_eq!(r.courses.len(), 1);
        assert_eq!(r.courses[0].confidence, Some(0.97));
        assert_eq!(r.unparsed.len(), 2);
        assert_eq!(r.unparsed[0].text, "2025-2026 学年第一学期课表");
        assert_eq!(r.unparsed[1].text, "周一 高数");
    }

    /// 离线验证桌面端整条数据通路：Umi-OCR 的 HTTP 响应 JSON -> OcrResponse -> OcrLine -> Course。
    /// 样本按 Umi-OCR 2.x 文档的 `data.format: dict` 结构手写（非实测抓包，见 README「Umi-OCR 实测」）。
    #[test]
    fn umi_ocr_response_shape_parses_into_courses() {
        let body = r#"{
            "code": 100,
            "data": [
                {"text": "2025-2026 学年第一学期课表", "score": 0.9912, "box": [[1,1],[2,1],[2,2],[1,2]], "end": "\n"},
                {"text": "高等数学 张老师 教一101 周一 1-2节 1-16周", "score": 0.9843, "box": [], "end": "\n"},
                {"text": "程序设计基础 李老师 教二203 周三 3-4节 1-16周", "score": 0.9721, "box": [], "end": "\n"},
                {"text": "大学英语 王老师 教三305 周五 5-6节 1-8单周", "score": 0.9555, "box": [], "end": ""}
            ],
            "time": 1.83,
            "timestamp": 1758230400.12
        }"#;

        let parsed: OcrResponse = serde_json::from_str(body).expect("响应结构应当能反序列化");
        assert_eq!(parsed.code, 100);
        let lines: Vec<OcrLine> = serde_json::from_value(parsed.data).expect("data 应当是文本块数组");
        assert_eq!(lines.len(), 4);

        let r = parse_lines(&lines);
        // 标题行认不出「星期 + 节次」→ 进 unparsed，不污染课表
        assert_eq!(r.unparsed.len(), 1);
        assert_eq!(r.unparsed[0].text, "2025-2026 学年第一学期课表");
        assert_eq!(r.courses.len(), 3);

        let math = &r.courses[0];
        assert_eq!(math.name, "高等数学");
        assert_eq!(math.teacher, "张老师");
        assert_eq!(math.room, "教一101");
        assert_eq!((math.day, math.start, math.end), (1, 1, 2));
        assert_eq!(math.weeks_text, "1-16");
        assert_eq!(math.weeks.len(), 16);

        let prog = &r.courses[1];
        assert_eq!(prog.name, "程序设计基础");
        assert_eq!(prog.room, "教二203");
        assert_eq!((prog.day, prog.start, prog.end), (3, 3, 4));

        let eng = &r.courses[2];
        assert_eq!(eng.name, "大学英语");
        assert_eq!((eng.day, eng.start, eng.end), (5, 5, 6));
        assert_eq!(eng.weeks, vec![1, 3, 5, 7]); // 1-8单周
        assert_eq!(eng.weeks_text, "1-8单");

        // 置信度都该高于阈值（不标黄）
        for c in &r.courses {
            assert!(c.confidence.unwrap() > LOW_CONFIDENCE, "{:?} 置信度偏低", c.name);
        }
    }

    #[test]
    fn umi_ocr_error_response_is_reported() {
        let body = r#"{"code": 902, "data": "向识别器进程传入指令失败，疑似子进程已崩溃"}"#;
        let parsed: OcrResponse = serde_json::from_str(body).unwrap();
        assert_ne!(parsed.code, 100);
        assert!(matches!(parsed.data, serde_json::Value::String(_)));
    }
}

/// 打开内置教务窗口。用户在里面自己登录，凭证只存在该 WebView 的内存里，我们不读、不存、不传。
#[tauri::command]
async fn open_admin(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("只支持 http/https 地址".into());
    }
    // 已经开着就聚焦，避免开出第二个登录窗口
    if let Some(w) = app.get_webview_window(ADMIN_LABEL) {
        let _ = w.set_focus();
        return Ok(());
    }

    let script = ADMIN_BUTTON_JS.to_string();
    let w = WebviewWindowBuilder::new(&app, ADMIN_LABEL, WebviewUrl::External(url.parse().map_err(|e| format!("地址不合法：{e}"))?))
        .title("教务系统 · 登录后点右下角「解析导入到课表」")
        .inner_size(1100.0, 800.0)
        .on_page_load(move |window, _payload| {
            let _ = window.eval(&script);
        })
        .build()
        .map_err(|e| format!("打不开教务窗口：{e}"))?;

    // 页面里的脚本会把 HTML 以 admin-html 事件发出来，主窗口负责调用 parse_zhengfang
    let _ = app.emit_to("main", "admin-opened", w.label());
    Ok(())
}

/// 解析教务课表 HTML（新版正方优先）。前端把内置窗口抓到的 HTML，或用户手动粘贴的 HTML 交给它。
#[tauri::command]
fn parse_zhengfang(html: String) -> admin::ParseScheduleResult {
    admin::parse_schedule(&html)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};

            let export = MenuItemBuilder::with_id("export-backup", "导出备份…").build(app)?;
            let import = MenuItemBuilder::with_id("import-backup", "导入备份…").build(app)?;
            let about = MenuItemBuilder::with_id("about", "关于课表").build(app)?;

            let submenu = SubmenuBuilder::new(app, "文件")
                .item(&export)
                .item(&import)
                .separator()
                .item(&PredefinedMenuItem::quit(app, Some("退出"))?)
                .build()?;

            let help = SubmenuBuilder::new(app, "帮助").item(&about).build()?;

            let menu = MenuBuilder::new(app).item(&submenu).item(&help).build()?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let _ = tauri::Emitter::emit(app, "menu", event.id().as_ref());
        })
        .invoke_handler(tauri::generate_handler![http_get, ocr_image, open_admin, parse_zhengfang])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
