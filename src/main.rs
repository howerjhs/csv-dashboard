#![windows_subsystem = "windows"]

use eframe::egui::{
    self, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, Sense, Shape,
    Stroke, Vec2,
};
use egui_plot::{Bar, BarChart, Legend, Line, Plot, PlotPoints};
use std::collections::{HashMap, HashSet};
use std::f32::consts::{FRAC_PI_2, TAU};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use std::sync::Mutex;

const PALETTE: [Color32; 10] = [
    Color32::from_rgb(66, 133, 244),
    Color32::from_rgb(234, 67, 53),
    Color32::from_rgb(251, 188, 5),
    Color32::from_rgb(52, 168, 83),
    Color32::from_rgb(171, 71, 188),
    Color32::from_rgb(0, 172, 193),
    Color32::from_rgb(255, 112, 67),
    Color32::from_rgb(158, 157, 36),
    Color32::from_rgb(92, 107, 192),
    Color32::from_rgb(240, 98, 146),
];

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let initial = std::env::args().nth(1);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CSV 대시보드")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([900.0, 600.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "CSV 대시보드",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, initial)))),
    )
}

/// 웹: index.html의 캔버스에 앱을 띄운다.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Warn).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().expect("window 없음").document().expect("document 없음");
        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("the_canvas_id 캔버스를 찾을 수 없습니다")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id 는 canvas 요소가 아닙니다");
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(App::new(cc)))),
            )
            .await;
        if let Some(loading) = document.get_element_by_id("loading_text") {
            match result {
                Ok(()) => loading.remove(),
                Err(e) => {
                    loading.set_inner_html("<p>앱을 시작하지 못했습니다. 개발자 도구 콘솔을 확인하세요.</p>");
                    panic!("eframe 시작 실패: {e:?}");
                }
            }
        }
    });
}

/// 기본 폰트에는 한글이 없으므로 한글 폰트를 대체 폰트로 등록한다.
fn add_korean_font(fonts: &mut FontDefinitions, data: FontData) {
    fonts.font_data.insert("korean".to_owned(), Arc::new(data));
    for fam in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(fam).or_default().push("korean".to_owned());
    }
}

/// 웹에는 시스템 폰트를 읽을 수 없으므로 서브셋한 나눔고딕(OFL)을 임베드한다.
#[cfg(target_arch = "wasm32")]
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    add_korean_font(
        &mut fonts,
        FontData::from_static(include_bytes!("../assets/fonts/DashboardKR-Regular.ttf")),
    );
    ctx.set_fonts(fonts);
}

/// 데스크톱은 Windows 시스템 한글 폰트를 사용한다.
#[cfg(not(target_arch = "wasm32"))]
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let candidates = [
        r"C:\Windows\Fonts\malgun.ttf",
        r"C:\Windows\Fonts\NanumGothic.ttf",
        r"C:\Windows\Fonts\gulim.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            add_korean_font(&mut fonts, FontData::from_owned(bytes));
            break;
        }
    }
    ctx.set_fonts(fonts);
}

// ---------------------------------------------------------------- 데이터

struct Stats {
    count: usize,
    sum: f64,
    mean: f64,
    min: f64,
    max: f64,
}

struct Column {
    name: String,
    values: Vec<String>,
    nums: Option<Vec<Option<f64>>>,
    stats: Option<Stats>,
}

struct Dataset {
    file_name: String,
    encoding: &'static str,
    delimiter: u8,
    n_rows: usize,
    cols: Vec<Column>,
}

impl Dataset {
    fn numeric_count(&self) -> usize {
        self.cols.iter().filter(|c| c.nums.is_some()).count()
    }
}

fn decode(bytes: &[u8]) -> (String, &'static str) {
    let bytes = bytes.strip_prefix([0xEFu8, 0xBB, 0xBF].as_slice()).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_string(), "UTF-8"),
        Err(_) => {
            // 한국어 엑셀에서 저장한 CSV는 대부분 CP949
            let (s, _, _) = encoding_rs::EUC_KR.decode(bytes);
            (s.into_owned(), "CP949")
        }
    }
}

fn parse_num(s: &str) -> Option<f64> {
    let t: String = s
        .chars()
        .filter(|c| !matches!(c, ',' | ' ' | '₩' | '$' | '%' | '원'))
        .collect();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn compute_stats(nums: &[Option<f64>]) -> Option<Stats> {
    let vals: Vec<f64> = nums.iter().flatten().copied().collect();
    if vals.is_empty() {
        return None;
    }
    let sum: f64 = vals.iter().sum();
    Some(Stats {
        count: vals.len(),
        sum,
        mean: sum / vals.len() as f64,
        min: vals.iter().copied().fold(f64::INFINITY, f64::min),
        max: vals.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    })
}

fn load_csv(bytes: &[u8], file_name: String) -> Result<Dataset, String> {
    let (text, encoding) = decode(bytes);
    let first_line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .ok_or("빈 파일입니다.")?;
    let delimiter = [b',', b';', b'\t', b'|']
        .into_iter()
        .max_by_key(|d| first_line.matches(*d as char).count())
        .unwrap();

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .has_headers(true)
        .from_reader(text.as_bytes());

    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("헤더를 읽을 수 없습니다: {e}"))?
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let h = h.trim();
            if h.is_empty() {
                format!("열{}", i + 1)
            } else {
                h.to_string()
            }
        })
        .collect();

    let mut raw: Vec<Vec<String>> = vec![Vec::new(); headers.len()];
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("CSV 파싱 오류: {e}"))?;
        if rec.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        for (i, col) in raw.iter_mut().enumerate() {
            col.push(rec.get(i).unwrap_or("").trim().to_string());
        }
    }
    let n_rows = raw.first().map_or(0, |c| c.len());
    if n_rows == 0 {
        return Err("데이터 행이 없습니다.".into());
    }

    let cols = headers
        .into_iter()
        .zip(raw)
        .map(|(name, values)| {
            let parsed: Vec<Option<f64>> = values.iter().map(|v| parse_num(v)).collect();
            let non_empty = values.iter().filter(|v| !v.is_empty()).count();
            let ok = parsed.iter().filter(|v| v.is_some()).count();
            // 비어있지 않은 값의 90% 이상이 숫자면 숫자형 열로 본다
            let nums = (ok > 0 && ok as f64 >= non_empty as f64 * 0.9).then_some(parsed);
            let stats = nums.as_deref().and_then(compute_stats);
            Column { name, values, nums, stats }
        })
        .collect();

    Ok(Dataset { file_name, encoding, delimiter, n_rows, cols })
}

// ---------------------------------------------------------------- 설정 / 파생 데이터

#[derive(Clone, Copy, PartialEq)]
enum Agg {
    Sum,
    Mean,
    Count,
}

#[derive(Clone, PartialEq)]
struct Settings {
    x_col: Option<usize>,
    y_cols: Vec<usize>,
    cat_col: Option<usize>,
    val_col: Option<usize>,
    agg: Agg,
    hist_col: Option<usize>,
    bins: usize,
    top_n: usize,
}

fn default_settings(ds: &Dataset) -> Settings {
    let numeric: Vec<usize> = (0..ds.cols.len()).filter(|&i| ds.cols[i].nums.is_some()).collect();
    let text: Vec<usize> = (0..ds.cols.len()).filter(|&i| ds.cols[i].nums.is_none()).collect();
    let distinct = |c: usize| ds.cols[c].values.iter().collect::<HashSet<_>>().len();

    // 값이 대부분 고유한 텍스트 열(날짜 등)은 X축으로
    let x_col = text
        .iter()
        .copied()
        .find(|&c| distinct(c) as f64 >= ds.n_rows as f64 * 0.8);
    let cat_col = text
        .iter()
        .copied()
        .filter(|&c| Some(c) != x_col)
        .find(|&c| (2..=50).contains(&distinct(c)))
        .or_else(|| text.iter().copied().find(|&c| Some(c) != x_col))
        .or(text.first().copied());

    Settings {
        x_col,
        y_cols: numeric.iter().take(3).copied().collect(),
        cat_col,
        val_col: numeric.first().copied(),
        agg: Agg::Sum,
        hist_col: numeric.first().copied(),
        bins: 20,
        top_n: 15,
    }
}

#[derive(Default)]
struct Derived {
    lines: Vec<(String, Vec<[f64; 2]>)>,
    x_labels: Option<Arc<Vec<String>>>,
    cats: Vec<(String, f64)>,
    hist: Vec<(f64, f64)>,
    hist_width: f64,
}

fn compute(ds: &Dataset, s: &Settings) -> Derived {
    let mut d = Derived::default();

    // 선 그래프
    let x_numeric = s.x_col.and_then(|c| ds.cols[c].nums.as_ref());
    if let Some(c) = s.x_col.filter(|&c| ds.cols[c].nums.is_none()) {
        d.x_labels = Some(Arc::new(ds.cols[c].values.clone()));
    }
    let step = (ds.n_rows / 4000).max(1);
    for &yc in &s.y_cols {
        let Some(ys) = ds.cols[yc].nums.as_ref() else { continue };
        let mut pts: Vec<[f64; 2]> = (0..ds.n_rows)
            .step_by(step)
            .filter_map(|i| {
                let y = ys[i]?;
                let x = match x_numeric {
                    Some(xs) => xs[i]?,
                    None => i as f64,
                };
                Some([x, y])
            })
            .collect();
        if x_numeric.is_some() {
            pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
        }
        d.lines.push((ds.cols[yc].name.clone(), pts));
    }

    // 분류별 집계
    if let Some(cc) = s.cat_col {
        let vals = s.val_col.and_then(|vc| ds.cols[vc].nums.as_ref());
        let mut map: HashMap<&str, (f64, usize)> = HashMap::new();
        for i in 0..ds.n_rows {
            let key = ds.cols[cc].values[i].as_str();
            let key = if key.is_empty() { "(빈 값)" } else { key };
            let v = match (s.agg, vals) {
                (Agg::Count, _) | (_, None) => Some(1.0),
                (_, Some(n)) => n[i],
            };
            if let Some(v) = v {
                let e = map.entry(key).or_insert((0.0, 0));
                e.0 += v;
                e.1 += 1;
            }
        }
        d.cats = map
            .into_iter()
            .map(|(k, (sum, cnt))| {
                let v = match s.agg {
                    Agg::Sum => sum,
                    Agg::Mean => sum / cnt as f64,
                    Agg::Count => cnt as f64,
                };
                (k.to_string(), v)
            })
            .collect();
        d.cats.sort_by(|a, b| b.1.total_cmp(&a.1));
    }

    // 히스토그램
    if let Some(hc) = s.hist_col {
        if let (Some(nums), Some(st)) = (&ds.cols[hc].nums, &ds.cols[hc].stats) {
            let bins = s.bins.max(1);
            let range = if st.max > st.min { st.max - st.min } else { 1.0 };
            d.hist_width = range / bins as f64;
            let mut counts = vec![0usize; bins];
            for v in nums.iter().flatten() {
                let b = (((v - st.min) / d.hist_width) as usize).min(bins - 1);
                counts[b] += 1;
            }
            d.hist = counts
                .iter()
                .enumerate()
                .map(|(i, c)| (st.min + d.hist_width * (i as f64 + 0.5), *c as f64))
                .collect();
        }
    }
    d
}

// ---------------------------------------------------------------- 앱

struct App {
    dataset: Option<Dataset>,
    settings: Option<Settings>,
    last_settings: Option<Settings>,
    derived: Derived,
    error: Option<String>,
    /// 웹 파일 선택 대화상자(비동기)의 결과를 다음 프레임에 넘겨받는 자리
    #[cfg(target_arch = "wasm32")]
    picked: Arc<Mutex<Option<(String, Vec<u8>)>>>,
}

impl App {
    fn empty(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        App {
            dataset: None,
            settings: None,
            last_settings: None,
            derived: Derived::default(),
            error: None,
            #[cfg(target_arch = "wasm32")]
            picked: Default::default(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::empty(cc)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn new(cc: &eframe::CreationContext<'_>, initial: Option<String>) -> Self {
        let mut app = Self::empty(cc);
        if let Some(p) = initial {
            app.open_path(Path::new(&p));
        }
        app
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_path(&mut self, p: &Path) {
        let name = p.file_name().map_or("파일".into(), |n| n.to_string_lossy().into_owned());
        match std::fs::read(p) {
            Ok(b) => self.open_bytes(&b, name),
            Err(e) => self.error = Some(format!("파일을 열 수 없습니다: {e}")),
        }
    }

    fn open_bytes(&mut self, bytes: &[u8], name: String) {
        match load_csv(bytes, name) {
            Ok(ds) => {
                let s = default_settings(&ds);
                self.derived = compute(&ds, &s);
                self.last_settings = Some(s.clone());
                self.settings = Some(s);
                self.dataset = Some(ds);
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn pick_file(&mut self, _ctx: &egui::Context) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("CSV 파일", &["csv", "tsv", "txt"])
            .add_filter("모든 파일", &["*"])
            .pick_file()
        {
            self.open_path(&p);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn pick_file(&mut self, ctx: &egui::Context) {
        let slot = self.picked.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(f) = rfd::AsyncFileDialog::new()
                .add_filter("CSV 파일", &["csv", "tsv", "txt"])
                .pick_file()
                .await
            {
                let bytes = f.read().await;
                *slot.lock().unwrap() = Some((f.file_name(), bytes));
                ctx.request_repaint();
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 드래그 앤 드롭
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(f) = dropped.first() {
            if let Some(b) = &f.bytes {
                self.open_bytes(b, f.name.clone());
            } else {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(p) = &f.path {
                    self.open_path(p);
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let picked = self.picked.lock().unwrap().take();
            if let Some((name, bytes)) = picked {
                self.open_bytes(&bytes, name);
            }
        }

        let mut want_open = false;
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(RichText::new("📊 CSV 대시보드").strong());
                ui.separator();
                if ui.button(RichText::new("📂 CSV 열기").size(15.0)).clicked() {
                    want_open = true;
                }
                if let Some(ds) = &self.dataset {
                    let delim = match ds.delimiter {
                        b'\t' => "탭".to_string(),
                        d => format!("'{}'", d as char),
                    };
                    ui.label(RichText::new(&ds.file_name).strong());
                    ui.label(
                        RichText::new(format!(
                            "{}행 × {}열 · 인코딩 {} · 구분자 {}",
                            fmt_num(ds.n_rows as f64),
                            ds.cols.len(),
                            ds.encoding,
                            delim
                        ))
                        .weak(),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_switch(ui);
                });
            });
            if let Some(err) = &self.error {
                ui.colored_label(Color32::from_rgb(234, 67, 53), format!("⚠ {err}"));
            }
            ui.add_space(4.0);
        });
        if want_open {
            self.pick_file(ctx);
        }

        match (&self.dataset, &mut self.settings) {
            (Some(ds), Some(s)) => {
                egui::SidePanel::left("controls")
                    .resizable(true)
                    .default_width(260.0)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| controls(ui, ds, s));
                    });
                if self.last_settings.as_ref() != Some(s) {
                    self.derived = compute(ds, s);
                    self.last_settings = Some(s.clone());
                }
                let (s, derived) = (&*s, &self.derived);
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| dashboard(ui, ds, s, derived));
                });
            }
            _ => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.3);
                        ui.label(RichText::new("📄").size(64.0));
                        ui.label(RichText::new("CSV 파일을 이 창에 끌어다 놓으세요").size(24.0).strong());
                        ui.add_space(8.0);
                        ui.label(RichText::new("숫자 열은 자동으로 인식되어 그래프로 표시됩니다 (UTF-8 / CP949 지원)").weak());
                        ui.add_space(16.0);
                        if ui.button(RichText::new("📂 파일 선택...").size(18.0)).clicked() {
                            want_open = true;
                        }
                    });
                });
                if want_open {
                    self.pick_file(ctx);
                }
            }
        }

        // 파일을 끌고 있을 때 오버레이
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("drop_overlay"),
            ));
            let rect = ctx.screen_rect();
            painter.rect_filled(rect, 0.0, Color32::from_black_alpha(170));
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "여기에 놓으면 대시보드를 만듭니다",
                FontId::proportional(28.0),
                Color32::WHITE,
            );
        }
    }
}

// ---------------------------------------------------------------- 사이드 패널

fn col_combo(
    ui: &mut egui::Ui,
    id: &str,
    ds: &Dataset,
    sel: &mut Option<usize>,
    none_label: Option<&str>,
    filter: impl Fn(&Column) -> bool,
) {
    let text = match sel {
        Some(c) => ds.cols[*c].name.clone(),
        None => none_label.unwrap_or("-").to_string(),
    };
    egui::ComboBox::from_id_salt(id)
        .selected_text(text)
        .width(ui.available_width() - 8.0)
        .show_ui(ui, |ui| {
            if let Some(n) = none_label {
                ui.selectable_value(sel, None, n);
            }
            for (i, c) in ds.cols.iter().enumerate() {
                if filter(c) {
                    ui.selectable_value(sel, Some(i), &c.name);
                }
            }
        });
}

fn controls(ui: &mut egui::Ui, ds: &Dataset, s: &mut Settings) {
    ui.add_space(6.0);
    ui.heading("📈 추이 그래프");
    ui.label("X축");
    col_combo(ui, "x", ds, &mut s.x_col, Some("(행 번호)"), |_| true);
    ui.label("Y축 (숫자 열)");
    for (i, c) in ds.cols.iter().enumerate() {
        if c.nums.is_none() {
            continue;
        }
        let mut on = s.y_cols.contains(&i);
        if ui.checkbox(&mut on, &c.name).changed() {
            if on {
                s.y_cols.push(i);
            } else {
                s.y_cols.retain(|&x| x != i);
            }
        }
    }

    ui.separator();
    ui.heading("📊 분류별 집계");
    ui.label("분류 기준 열");
    col_combo(ui, "cat", ds, &mut s.cat_col, Some("(없음)"), |_| true);
    ui.label("집계 방식");
    ui.horizontal(|ui| {
        ui.radio_value(&mut s.agg, Agg::Sum, "합계");
        ui.radio_value(&mut s.agg, Agg::Mean, "평균");
        ui.radio_value(&mut s.agg, Agg::Count, "개수");
    });
    ui.add_enabled_ui(s.agg != Agg::Count, |ui| {
        ui.label("값 열");
        col_combo(ui, "val", ds, &mut s.val_col, Some("(개수)"), |c| c.nums.is_some());
    });
    ui.add(egui::Slider::new(&mut s.top_n, 3..=40).text("상위 N개"));

    ui.separator();
    ui.heading("📶 분포 (히스토그램)");
    col_combo(ui, "hist", ds, &mut s.hist_col, Some("(없음)"), |c| c.nums.is_some());
    ui.add(egui::Slider::new(&mut s.bins, 5..=100).text("구간 수"));
}

// ---------------------------------------------------------------- 대시보드

fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "-".into();
    }
    if v.abs() >= 1e15 || (v != 0.0 && v.abs() < 1e-3) {
        return format!("{v:.3e}");
    }
    let decimals = if v.fract().abs() < 1e-9 {
        0
    } else if v.abs() >= 100.0 {
        1
    } else {
        2
    };
    let s = format!("{:.*}", decimals, v.abs());
    let (int, frac) = match s.split_once('.') {
        Some((a, b)) => (a.to_string(), format!(".{b}")),
        None => (s.clone(), String::new()),
    };
    let mut out = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{}{}{}", if v < 0.0 { "-" } else { "" }, out, frac)
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        format!("{}…", s.chars().take(n).collect::<String>())
    } else {
        s.to_string()
    }
}

fn card(ui: &mut egui::Ui, title: &str, value: String, sub: String, color: Color32) {
    egui::Frame::group(ui.style()).inner_margin(12.0).show(ui, |ui| {
        ui.set_min_width(190.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(title).weak());
            ui.label(RichText::new(value).size(26.0).strong().color(color));
            if !sub.is_empty() {
                ui.label(RichText::new(sub).small());
            }
        });
    });
}

fn section<R>(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) {
    egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).size(16.0).strong());
        ui.add_space(4.0);
        add(ui);
    });
}

fn dashboard(ui: &mut egui::Ui, ds: &Dataset, s: &Settings, d: &Derived) {
    // KPI 카드
    ui.horizontal_wrapped(|ui| {
        card(ui, "전체 행 수", fmt_num(ds.n_rows as f64), String::new(), PALETTE[0]);
        card(
            ui,
            "열 수",
            ds.cols.len().to_string(),
            format!("숫자형 {} · 텍스트 {}", ds.numeric_count(), ds.cols.len() - ds.numeric_count()),
            PALETTE[3],
        );
        for (k, &c) in s.y_cols.iter().take(4).enumerate() {
            if let Some(st) = &ds.cols[c].stats {
                card(
                    ui,
                    &format!("{} 합계", ds.cols[c].name),
                    fmt_num(st.sum),
                    format!("평균 {} · 최소 {} · 최대 {}", fmt_num(st.mean), fmt_num(st.min), fmt_num(st.max)),
                    PALETTE[k % PALETTE.len()],
                );
            }
        }
    });
    ui.add_space(8.0);

    // 추이 그래프
    section(ui, "📈 추이", |ui| {
        if d.lines.is_empty() {
            ui.label("왼쪽에서 Y축 숫자 열을 선택하세요.");
            return;
        }
        let mut plot = Plot::new("line")
            .height(320.0)
            .legend(Legend::default())
            .allow_scroll(false);
        if let Some(labels) = d.x_labels.clone() {
            plot = plot.x_axis_formatter(move |mark, _range| {
                let v = mark.value;
                if v >= 0.0 && (v - v.round()).abs() < 1e-6 {
                    labels.get(v.round() as usize).map(|l| truncate(l, 12)).unwrap_or_default()
                } else {
                    String::new()
                }
            });
        }
        plot.show(ui, |pui| {
            for (i, (name, pts)) in d.lines.iter().enumerate() {
                pui.line(
                    Line::new(PlotPoints::from(pts.clone()))
                        .name(name)
                        .color(PALETTE[i % PALETTE.len()])
                        .width(2.0_f32),
                );
            }
        });
    });
    ui.add_space(8.0);

    // 분류별 막대 + 도넛
    let cat_title = match (s.cat_col, s.agg, s.val_col) {
        (None, _, _) => "📊 분류별 집계".to_string(),
        (Some(c), Agg::Count, _) | (Some(c), _, None) => format!("📊 {}별 개수", ds.cols[c].name),
        (Some(c), agg, Some(v)) => format!(
            "📊 {}별 {} {}",
            ds.cols[c].name,
            ds.cols[v].name,
            if agg == Agg::Sum { "합계" } else { "평균" }
        ),
    };
    ui.columns(2, |cols| {
        section(&mut cols[0], &cat_title, |ui| {
            if d.cats.is_empty() {
                ui.label("분류 기준 열을 선택하세요.");
                return;
            }
            let top: Vec<&(String, f64)> = d.cats.iter().take(s.top_n).collect();
            let names: Arc<Vec<String>> = Arc::new(top.iter().map(|(k, _)| k.clone()).collect());
            let bars: Vec<Bar> = top
                .iter()
                .enumerate()
                .map(|(i, (k, v))| {
                    Bar::new(i as f64, *v)
                        .name(k)
                        .width(0.7)
                        .fill(PALETTE[i % PALETTE.len()])
                })
                .collect();
            Plot::new("bar")
                .height(300.0)
                .allow_scroll(false)
                .x_axis_formatter(move |mark, _range| {
                    let v = mark.value;
                    if v >= 0.0 && (v - v.round()).abs() < 1e-6 {
                        names.get(v.round() as usize).map(|l| truncate(l, 8)).unwrap_or_default()
                    } else {
                        String::new()
                    }
                })
                .show(ui, |pui| pui.bar_chart(BarChart::new(bars)));
        });
        section(&mut cols[1], "🍩 비중", |ui| {
            if d.cats.is_empty() {
                ui.label("분류 기준 열을 선택하세요.");
            } else if s.agg == Agg::Mean {
                ui.label("평균 집계에서는 비중 차트를 표시하지 않습니다.");
            } else {
                let mut pie: Vec<(String, f64)> =
                    d.cats.iter().filter(|c| c.1 > 0.0).take(7).cloned().collect();
                let rest: f64 = d.cats.iter().filter(|c| c.1 > 0.0).skip(7).map(|c| c.1).sum();
                if rest > 0.0 {
                    pie.push(("기타".into(), rest));
                }
                donut(ui, &pie);
            }
        });
    });
    ui.add_space(8.0);

    // 히스토그램 + 요약 통계
    ui.columns(2, |cols| {
        let hist_title = match s.hist_col {
            Some(c) => format!("📶 {} 분포", ds.cols[c].name),
            None => "📶 분포".into(),
        };
        section(&mut cols[0], &hist_title, |ui| {
            if d.hist.is_empty() {
                ui.label("숫자 열을 선택하세요.");
                return;
            }
            let bars: Vec<Bar> = d
                .hist
                .iter()
                .map(|(x, c)| Bar::new(*x, *c).width(d.hist_width * 0.95).fill(PALETTE[5]))
                .collect();
            Plot::new("hist")
                .height(280.0)
                .allow_scroll(false)
                .show(ui, |pui| pui.bar_chart(BarChart::new(bars)));
        });
        section(&mut cols[1], "🧮 숫자 열 요약", |ui| {
            egui::ScrollArea::both().id_salt("summary").max_height(280.0).show(ui, |ui| {
                egui::Grid::new("stats").striped(true).spacing([16.0, 6.0]).show(ui, |ui| {
                    for h in ["열", "개수", "합계", "평균", "최소", "최대"] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for c in &ds.cols {
                        if let Some(st) = &c.stats {
                            ui.label(truncate(&c.name, 20));
                            ui.label(fmt_num(st.count as f64));
                            ui.label(fmt_num(st.sum));
                            ui.label(fmt_num(st.mean));
                            ui.label(fmt_num(st.min));
                            ui.label(fmt_num(st.max));
                            ui.end_row();
                        }
                    }
                });
            });
        });
    });
    ui.add_space(8.0);

    // 데이터 미리보기
    let preview = ds.n_rows.min(100);
    egui::CollapsingHeader::new(RichText::new(format!("📋 데이터 미리보기 (처음 {preview}행)")).size(16.0).strong())
        .default_open(false)
        .show(ui, |ui| {
            egui::ScrollArea::horizontal().id_salt("preview").show(ui, |ui| {
                egui::Grid::new("preview_grid").striped(true).spacing([14.0, 4.0]).show(ui, |ui| {
                    ui.label(RichText::new("#").strong());
                    for c in &ds.cols {
                        ui.label(RichText::new(truncate(&c.name, 20)).strong());
                    }
                    ui.end_row();
                    for r in 0..preview {
                        ui.label(RichText::new((r + 1).to_string()).weak());
                        for c in &ds.cols {
                            ui.label(truncate(&c.values[r], 30));
                        }
                        ui.end_row();
                    }
                });
            });
        });
    ui.add_space(16.0);
}

fn donut(ui: &mut egui::Ui, data: &[(String, f64)]) {
    let total: f64 = data.iter().map(|d| d.1).sum();
    if total <= 0.0 {
        ui.label("표시할 양수 값이 없습니다.");
        return;
    }
    ui.horizontal(|ui| {
        let size = 260.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        let painter = ui.painter_at(rect);
        let c = rect.center();
        let (r_in, r_out) = (size * 0.27, size * 0.44);

        // 마우스가 올라간 조각 찾기 (12시 방향부터 시계방향)
        let hovered = resp.hover_pos().and_then(|p| {
            let v = p - c;
            let dist = v.length();
            if dist < r_in || dist > r_out + 8.0 {
                return None;
            }
            let rel = (v.y.atan2(v.x) + FRAC_PI_2).rem_euclid(TAU);
            let mut acc = 0.0;
            data.iter().position(|(_, val)| {
                acc += (*val / total) as f32 * TAU;
                rel < acc
            })
        });

        let mut start = -FRAC_PI_2;
        for (i, (_, v)) in data.iter().enumerate() {
            let sweep = (*v / total) as f32 * TAU;
            let rr = if hovered == Some(i) { r_out + 8.0 } else { r_out };
            let color = PALETTE[i % PALETTE.len()];
            let segs = ((sweep / 0.05).ceil() as usize).max(1);
            for k in 0..segs {
                let a0 = start + sweep * k as f32 / segs as f32;
                let a1 = start + sweep * (k + 1) as f32 / segs as f32;
                painter.add(Shape::convex_polygon(
                    vec![
                        c + Vec2::angled(a0) * r_in,
                        c + Vec2::angled(a0) * rr,
                        c + Vec2::angled(a1) * rr,
                        c + Vec2::angled(a1) * r_in,
                    ],
                    color,
                    Stroke::new(1.0_f32, color),
                ));
            }
            start += sweep;
        }

        let text_color = ui.visuals().strong_text_color();
        let (label, value) = match hovered {
            Some(i) => (truncate(&data[i].0, 10), format!("{:.1}%", data[i].1 / total * 100.0)),
            None => ("합계".to_string(), fmt_num(total)),
        };
        painter.text(c - Vec2::new(0.0, 10.0), Align2::CENTER_CENTER, label, FontId::proportional(14.0), text_color);
        painter.text(c + Vec2::new(0.0, 12.0), Align2::CENTER_CENTER, value, FontId::proportional(18.0), text_color);

        ui.vertical(|ui| {
            ui.add_space(20.0);
            for (i, (k, v)) in data.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                    ui.painter().rect_filled(r, 2.0, PALETTE[i % PALETTE.len()]);
                    let text = format!("{}  {:.1}%", truncate(k, 16), v / total * 100.0);
                    if hovered == Some(i) {
                        ui.label(RichText::new(text).strong());
                    } else {
                        ui.label(text);
                    }
                });
            }
        });
    });
}
