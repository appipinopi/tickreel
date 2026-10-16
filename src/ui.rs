use crate::{
    audio::{self, Microphone, Player},
    engine::{Engine, Snapshot},
    export,
    model::{Edit, Recording, parse_captions, period_records, selected_frames},
    native::{self, Monitor, Window},
};
use anyhow::{Context, Result};
use eframe::egui::{self, Color32, RichText, TextureHandle, Vec2};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
const GREEN: Color32 = Color32::from_rgb(25, 124, 112);
enum Page {
    Today,
    Library,
}
struct Editor {
    record: Recording,
    captions: String,
    indices: Vec<u32>,
    position: f64,
    playing: Option<(Instant, f64)>,
    texture: Option<TextureHandle>,
    image_index: Option<u32>,
    microphone: Option<Microphone>,
    voice_temp: Option<tempfile::TempDir>,
    player: Option<Player>,
    decoded_voice: Option<tempfile::TempDir>,
}
struct Desktop {
    engine: Engine,
    page: Page,
    monitors: Vec<Monitor>,
    windows: Vec<Window>,
    options: Recording,
    records: Vec<Recording>,
    editor: Option<Editor>,
    texture: Option<TextureHandle>,
    revision: u64,
    refresh: Instant,
    message: String,
    last_engine_message: Option<String>,
    summary_date: String,
    month: bool,
    summary_theme: String,
    delete_id: Option<String>,
    close_confirm: bool,
    allow_close: bool,
    import: mpsc::Receiver<Result<usize>>,
    image_refresh: Instant,
    qa_frames: Option<u32>,
    _qa_root: Option<tempfile::TempDir>,
}
pub fn run() -> Result<()> {
    let qa = std::env::args().any(|argument| argument.starts_with("--ui-test"));
    let qa_root = if qa { Some(tempfile::tempdir()?) } else { None };
    let root = if let Some(root) = &qa_root {
        root.path().join("records")
    } else {
        dirs::data_dir()
            .context("データフォルダーが見つかりません。")?
            .join("TickreelRust/recordings")
    };
    let engine = Engine::new(root)?;
    let (tx, rx) = mpsc::channel();
    let store = engine.store.clone();
    let legacy = dirs::data_dir().unwrap().join("tickreel/recordings");
    std::thread::spawn(move || {
        let _ = tx.send(if qa {
            Ok(0)
        } else {
            store.import_legacy(&legacy)
        });
    });
    let monitors = native::monitors()?;
    let windows = native::windows().unwrap_or_default();
    let mut options = Recording::default();
    options.title.clear();
    options.source_ids = monitors
        .first()
        .map(|monitor| vec![monitor.id.clone()])
        .unwrap_or_default();
    if crate::capture_platform::is_wayland() {
        options.idle_speed = 1;
    }
    let desktop = Desktop {
        engine,
        page: Page::Today,
        monitors,
        windows: if qa { vec![] } else { windows },
        options,
        records: vec![],
        editor: None,
        texture: None,
        revision: 0,
        refresh: Instant::now() - Duration::from_secs(5),
        message: String::new(),
        last_engine_message: None,
        summary_date: chrono::Local::now().date_naive().to_string(),
        month: false,
        summary_theme: "cinema".into(),
        delete_id: None,
        close_confirm: false,
        allow_close: false,
        import: rx,
        image_refresh: Instant::now(),
        qa_frames: if qa { Some(0) } else { None },
        _qa_root: qa_root,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!(
                "../assets/tickreel.png"
            ))?)
            .with_inner_size([1320.0, 940.0])
            .with_min_inner_size([1020.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tickreel",
        options,
        Box::new(move |creation| {
            let mut fonts = egui::FontDefinitions::default();
            for file in [
                "C:/Windows/Fonts/YuGothR.ttc",
                "C:/Windows/Fonts/meiryo.ttc",
                "C:/Windows/Fonts/msgothic.ttc",
            ] {
                if let Ok(bytes) = fs::read(file) {
                    fonts.font_data.insert(
                        "japanese".into(),
                        std::sync::Arc::new(egui::FontData::from_owned(bytes)),
                    );
                    fonts
                        .families
                        .entry(egui::FontFamily::Proportional)
                        .or_default()
                        .insert(0, "japanese".into());
                    fonts
                        .families
                        .entry(egui::FontFamily::Monospace)
                        .or_default()
                        .push("japanese".into());
                    break;
                }
            }
            creation.egui_ctx.set_fonts(fonts);
            creation.egui_ctx.set_visuals(egui::Visuals::light());
            creation.egui_ctx.style_mut(|style| {
                style
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
                style
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                style
                    .text_styles
                    .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
                style
                    .text_styles
                    .insert(egui::TextStyle::Heading, egui::FontId::proportional(24.0));
                style.spacing.item_spacing = Vec2::new(10.0, 10.0);
                style.spacing.button_padding = Vec2::new(15.0, 8.0);
                style.visuals.panel_fill = Color32::from_rgb(247, 249, 250);
                style.visuals.selection.bg_fill = GREEN;
                style.visuals.widgets.active.bg_fill = GREEN;
                style.visuals.override_text_color = Some(Color32::from_rgb(38, 57, 52));
                style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(246, 250, 248);
                style.visuals.widgets.inactive.bg_stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(210, 226, 219));
                style.visuals.widgets.noninteractive.bg_stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(210, 226, 219));
            });
            let mut desktop = desktop;
            if std::env::args().any(|argument| argument == "--ui-test-editor") {
                let mut record = Recording {
                    title: "イラスト制作の一日".into(),
                    active_ms: 3_600_000,
                    status: "complete".into(),
                    source_ids: vec!["test".into()],
                    ..Default::default()
                };
                desktop.engine.store.save(&record)?;
                let sample = image::load_from_memory(export::END_CARD)?.to_rgb8();
                desktop.engine.store.frame(&mut record, &sample, false)?;
                record.edit.captions = parse_captions("0:00 | 今日の制作スタート")?;
                desktop.engine.store.save(&record)?;
                desktop.open_editor(record, &creation.egui_ctx)?;
            }
            Ok(Box::new(desktop))
        }),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}
fn choice<T: PartialEq + Copy + std::fmt::Display>(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    current: &mut T,
    values: &[(T, &str)],
) {
    ui.label(label);
    egui::ComboBox::from_id_salt(id)
        .selected_text(
            values
                .iter()
                .find(|(value, _)| value == current)
                .map_or("", |(_, name)| *name),
        )
        .width(ui.available_width().min(320.0))
        .show_ui(ui, |ui| {
            for (value, name) in values {
                ui.selectable_value(current, *value, *name);
            }
        });
}
fn string_choice(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    current: &mut String,
    values: &[(&str, &str)],
) {
    ui.label(label);
    egui::ComboBox::from_id_salt(id)
        .selected_text(
            values
                .iter()
                .find(|(value, _)| *value == current)
                .map_or("", |(_, name)| *name),
        )
        .width(ui.available_width().min(320.0))
        .show_ui(ui, |ui| {
            for (value, name) in values {
                ui.selectable_value(current, (*value).into(), *name);
            }
        });
}
fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(225, 233, 232)))
        .corner_radius(10)
        .inner_margin(18)
        .show(ui, content);
}
fn primary(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(GREEN))
}
fn texture(ctx: &egui::Context, image: &image::RgbImage, name: &str) -> TextureHandle {
    let scaled = image::imageops::thumbnail(image, 1100, 620);
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgb(
            [scaled.width() as usize, scaled.height() as usize],
            scaled.as_raw(),
        ),
        egui::TextureOptions::LINEAR,
    )
}
fn photo(ui: &mut egui::Ui, texture: Option<&TextureHandle>, width: f32) -> egui::Rect {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(width, width * 9.0 / 16.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 6, Color32::from_rgb(227, 239, 234));
    if let Some(texture) = texture {
        let size = texture.size_vec2();
        let scale = (rect.width() / size.x).min(rect.height() / size.y);
        let image_rect = egui::Rect::from_center_size(rect.center(), size * scale);
        ui.painter().image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "ここから、今日を記録しよう。",
            egui::FontId::proportional(20.0),
            GREEN,
        );
    }
    rect
}
impl Desktop {
    fn result<T>(&mut self, result: Result<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.message = format!("{error:#}");
                None
            }
        }
    }
    fn today(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, snapshot: &Snapshot) {
        ui.label(
            RichText::new("MAKE YOUR DAY A REEL")
                .color(GREEN)
                .size(11.0),
        );
        ui.heading("一日の積み重ねを、一本に。");
        ui.label("いつもの作業を、あとから見返せるタイムラプスに。");
        ui.add_space(15.0);
        let width = ui.available_width();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width((width - 330.0).max(360.0));
                card(ui, |ui| {
                    ui.label(RichText::new("スクリーンプレビュー").strong());
                    photo(ui, self.texture.as_ref(), ui.available_width());
                    ui.horizontal(|ui| {
                        if let Some(record) = &snapshot.current {
                            ui.label(format!(
                                "記録 {}分 {}秒",
                                record.active_ms / 60000,
                                record.active_ms / 1000 % 60
                            ));
                            ui.separator();
                            ui.label(format!(
                                "{}枚 · 本編 約{:.1}秒",
                                record.frames,
                                record.frames as f64 / record.fps as f64
                            ));
                        } else {
                            ui.label("記録の準備ができています");
                        }
                    });
                    ui.horizontal(|ui| {
                        if let Some(record) = &snapshot.current {
                            if primary(ui, "■ 記録を終了").clicked() {
                                let result = self.engine.stop();
                                if self.result(result).is_some() {
                                    self.message =
                                        "記録を保存しました。ワンクリック生成で動画にできます。"
                                            .into();
                                }
                            }
                            if ui
                                .button(if record.status == "paused" {
                                    "▶ 再開"
                                } else {
                                    "Ⅱ 一時停止"
                                })
                                .clicked()
                            {
                                let result = if record.status == "paused" {
                                    self.engine.resume()
                                } else {
                                    self.engine.pause()
                                };
                                self.result(result);
                            }
                        } else if primary(ui, "● 記録を開始").clicked() {
                            let result = self.engine.start(self.options.clone());
                            self.result(result);
                        }
                    });
                });
            });
            ui.vertical(|ui| {
                ui.set_width(300.0);
                card(ui, |ui| {
                    ui.heading("記録の設定");
                    ui.add_enabled_ui(snapshot.current.is_none(), |ui| {
                        ui.label("記録のタイトル");
                        ui.text_edit_singleline(&mut self.options.title);
                        string_choice(
                            ui,
                            "category",
                            "制作の種類",
                            &mut self.options.category,
                            &[
                                ("drawing", "お絵描き · Paper"),
                                ("development", "開発 · Terminal"),
                            ],
                        );
                        choice(
                            ui,
                            "interval",
                            "キャプチャ間隔",
                            &mut self.options.interval,
                            &[
                                (1, "1秒"),
                                (5, "5秒"),
                                (10, "10秒"),
                                (30, "30秒"),
                                (60, "60秒"),
                            ],
                        );
                        choice(
                            ui,
                            "fps",
                            "動画のフレームレート",
                            &mut self.options.fps,
                            &[
                                (15, "15 fps"),
                                (24, "24 fps"),
                                (30, "30 fps"),
                                (60, "60 fps"),
                            ],
                        );
                        if ui.button("1分 → 約4秒（15倍速）").clicked() {
                            self.options.interval = 1;
                            self.options.fps = 15;
                            self.options.idle_speed = 1;
                        }
                        ui.separator();
                        ui.label("記録するモニター（最大8画面）");
                        #[cfg(target_os = "linux")]
                        if crate::capture_platform::is_wayland() {
                            ui.small(
                                "Waylandではアプリ別の黒塗り・無操作の自動早送りは利用できません。",
                            );
                            if ui.button("共有する画面を選ぶ").clicked() {
                                let result =
                                    crate::wayland::select().and_then(|()| native::monitors());
                                if let Some(monitors) = self.result(result) {
                                    self.options.source_ids =
                                        monitors.iter().map(|m| m.id.clone()).collect();
                                    self.monitors = monitors;
                                }
                            }
                            if ui.button("画面共有を終了").clicked() {
                                crate::wayland::release();
                                self.monitors.clear();
                                self.options.source_ids.clear();
                            }
                        }
                        for monitor in &self.monitors {
                            let mut selected = self.options.source_ids.contains(&monitor.id);
                            if ui.checkbox(&mut selected, &monitor.name).changed() {
                                if selected {
                                    self.options.source_ids.push(monitor.id.clone());
                                } else {
                                    self.options.source_ids.retain(|id| id != &monitor.id);
                                }
                            }
                        }
                        if ui.small_button("画面一覧を更新").clicked() {
                            let result = native::monitors();
                            if let Some(monitors) = self.result(result) {
                                self.monitors = monitors;
                            }
                        }
                        string_choice(
                            ui,
                            "layout",
                            "並べ方",
                            &mut self.options.layout,
                            &[("grid", "グリッド"), ("horizontal", "横に並べる")],
                        );
                    });
                    ui.separator();
                    ui.label(format!(
                        "8時間 → 約{:.0}秒 ＋ エンドカード3秒",
                        28800.0 / self.options.interval as f64 / self.options.fps as f64
                    ));
                    ui.small("ロック・スリープ時は自動で一時停止します。");
                });
            });
        });
        ui.add_space(16.0);
        card(ui, |ui| {
            ui.heading("プライバシーと無操作の早送り");
            ui.add_enabled_ui(snapshot.current.is_none() && !crate::capture_platform::is_wayland(),|ui|{ui.horizontal(|ui|{if ui.button("アプリ一覧を更新").clicked(){let result=native::windows();if let Some(windows)=self.result(result){self.windows=windows;}}ui.label("指定アプリの表示中のウィンドウを保存前に黒塗り");});
            let mut apps:BTreeMap<String,String>=BTreeMap::new();for window in &self.windows{if !window.title.is_empty(){apps.entry(window.process.clone()).or_insert(window.title.clone());}}for selected in &self.options.protected_apps{apps.entry(selected.clone()).or_insert("現在ウィンドウなし".into());}
            egui::ScrollArea::vertical().id_salt("privacy").max_height(140.0).show(ui,|ui|{for(process,title)in apps{let mut selected=self.options.protected_apps.contains(&process);if ui.checkbox(&mut selected,format!("{process} · {title}")).changed(){if selected{self.options.protected_apps.push(process);}else{self.options.protected_apps.retain(|name|name!=&process);}}}});
            ui.horizontal(|ui|{choice(ui,"threshold","無操作の判定",&mut self.options.idle_threshold,&[(60,"1分"),(120,"2分"),(300,"5分")]);choice(ui,"idle","追加の早送り",&mut self.options.idle_speed,&[(1,"なし"),(4,"4倍"),(8,"8倍"),(16,"16倍")]);});ui.small("位置検出に失敗すると記録を停止します。ビルド・描画の待ち時間も無操作に含まれます。");});
        });
        ui.add_space(20.0);
        ui.heading("最近の記録");
        self.record_list(ui, ctx, snapshot, 3);
    }
    fn record_list(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        snapshot: &Snapshot,
        limit: usize,
    ) {
        if self.records.is_empty() {
            ui.label("まだ記録はありません。最初の一日を、ここに残しましょう。");
        }
        for record in self.records.clone().into_iter().take(limit) {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(300.0);
                        ui.label(RichText::new(&record.title).strong());
                        ui.small(format!(
                            "{} · {}枚 · 作業 {}分",
                            record.local_day(),
                            record.frames,
                            record.active_ms.saturating_sub(record.idle_ms) / 60000
                        ));
                    });
                    let active = snapshot
                        .current
                        .as_ref()
                        .is_some_and(|current| current.id == record.id);
                    let enabled = !active && snapshot.export.is_none();
                    ui.add_enabled_ui(enabled, |ui| {
                        if ui
                            .add_enabled(record.frames > 0, egui::Button::new("編集"))
                            .clicked()
                        {
                            let result = self.open_editor(record.clone(), ctx);
                            self.result(result);
                        }
                        if ui
                            .add_enabled(
                                record.frames > 0,
                                egui::Button::new(
                                    RichText::new("✦ ワンクリック生成").color(Color32::WHITE),
                                )
                                .fill(GREEN),
                            )
                            .clicked()
                        {
                            let destination = self.engine.quick_destination(&record.title);
                            let result = self.engine.export(
                                vec![record.clone()],
                                destination,
                                record.edit.clone(),
                            );
                            self.result(result);
                        }
                        if ui
                            .button(RichText::new("削除").color(Color32::from_rgb(165, 83, 78)))
                            .clicked()
                        {
                            self.delete_id = Some(record.id.clone());
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }
    }
    fn library(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, snapshot: &Snapshot) {
        ui.heading("ライブラリ");
        ui.label("テロップも声も、一週間の積み重ねも。あなたの制作日記に。");
        ui.add_space(14.0);
        card(ui, |ui| {
            ui.heading("週間・月間まとめ");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.month, false, "週間（月曜〜日曜）");
                ui.selectable_value(&mut self.month, true, "月間");
                ui.add(egui::TextEdit::singleline(&mut self.summary_date).desired_width(140.0));
            });
            string_choice(
                ui,
                "summary-theme",
                "まとめのテーマ",
                &mut self.summary_theme,
                &[
                    ("cinema", "Cinema"),
                    ("paper", "Paper"),
                    ("terminal", "Terminal"),
                    ("clean", "Clean"),
                ],
            );
            let records = chrono::NaiveDate::parse_from_str(&self.summary_date, "%Y-%m-%d")
                .map(|date| period_records(&self.records, date, self.month))
                .unwrap_or_default();
            ui.label(format!(
                "{}件の保存済み記録を、時系列で結合します。",
                records.len()
            ));
            ui.add_enabled_ui(!records.is_empty() && snapshot.export.is_none(), |ui| {
                if primary(ui, "✦ まとめを生成").clicked() {
                    let title = format!(
                        "{}まとめ-{}",
                        if self.month { "月間" } else { "週間" },
                        self.summary_date
                    );
                    let destination = self.engine.quick_destination(&title);
                    let result = self.engine.export(
                        records,
                        destination,
                        Edit {
                            theme: self.summary_theme.clone(),
                            ..Default::default()
                        },
                    );
                    self.result(result);
                }
            });
        });
        ui.add_space(20.0);
        self.record_list(ui, ctx, snapshot, usize::MAX);
    }
    fn open_editor(&mut self, record: Recording, ctx: &egui::Context) -> Result<()> {
        let indices = selected_frames(&record, &self.engine.store.timeline(&record.id)?);
        let image = image::open(
            self.engine
                .store
                .directory(&record.id)?
                .join(format!("frame-{:07}.jpg", indices[0])),
        )?
        .to_rgb8();
        let captions = record
            .edit
            .captions
            .iter()
            .map(|caption| {
                format!(
                    "{:.1}-{:.1} | {}",
                    caption.start,
                    caption.start + caption.duration,
                    caption.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.editor = Some(Editor {
            record,
            captions,
            indices,
            position: 0.0,
            playing: None,
            texture: Some(texture(ctx, &image, "editor")),
            image_index: Some(0),
            microphone: None,
            voice_temp: None,
            player: None,
            decoded_voice: None,
        });
        Ok(())
    }
    fn finish_voice(&mut self, editor: &mut Editor) -> Result<()> {
        if let Some(mic) = editor.microphone.take() {
            mic.finish()?;
            let temp = editor
                .voice_temp
                .as_ref()
                .context("録音の保存先がありません。")?
                .path()
                .join("voice.wav");
            let destination = self
                .engine
                .store
                .directory(&editor.record.id)?
                .join("narration.wav");
            audio::commit_audio(&temp, &destination)?;
            editor.record.narration_path = Some(destination.to_string_lossy().into_owned());
            self.engine.save_edit(&editor.record)?;
            editor.voice_temp = None;
            editor.decoded_voice = None;
            self.message = "アフレコを保存しました。".into();
        }
        Ok(())
    }
    fn save_editor(&mut self, editor: &mut Editor) -> Result<()> {
        editor.record.edit.captions = parse_captions(&editor.captions)?;
        self.engine.save_edit(&editor.record)
    }
    fn editor(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, snapshot: &Snapshot) {
        let Some(mut editor) = self.editor.take() else {
            return;
        };
        let mut close = false;
        ui.horizontal(|ui| {
            ui.heading(&editor.record.title);
            if ui.button("← 一覧に戻る").clicked() {
                if editor.microphone.is_some() {
                    self.message = "録音を停止して保存してから閉じてください。".into();
                } else {
                    close = true;
                }
            }
        });
        let duration = editor.indices.len() as f64 / editor.record.fps as f64;
        let total = duration + 3.0;
        if let Some((started, position)) = editor.playing {
            editor.position = (position + started.elapsed().as_secs_f64()).min(total);
            if editor.position >= total {
                editor.playing = None;
                editor.player = None;
            }
        }
        if editor
            .microphone
            .as_ref()
            .is_some_and(Microphone::should_stop)
        {
            let result = self.finish_voice(&mut editor);
            self.result(result);
        }
        let end = editor.position >= duration;
        let index = if end {
            None
        } else {
            editor
                .indices
                .get((editor.position * editor.record.fps as f64) as usize)
                .copied()
        };
        if index != editor.image_index && self.image_refresh.elapsed() > Duration::from_millis(150)
        {
            let result = if let Some(index) = index {
                image::open(
                    self.engine
                        .store
                        .directory(&editor.record.id)
                        .unwrap()
                        .join(format!("frame-{index:07}.jpg")),
                )
                .map(|image| image.to_rgb8())
                .map_err(anyhow::Error::from)
            } else {
                image::load_from_memory(export::END_CARD)
                    .map(|image| image.to_rgb8())
                    .map_err(anyhow::Error::from)
            };
            if let Some(image) = self.result(result) {
                editor.texture = Some(texture(ctx, &image, "editor"));
                editor.image_index = index;
            }
            self.image_refresh = Instant::now();
        }
        let width = ui.available_width();
        ui.horizontal_top(|ui|{ui.vertical(|ui|{ui.set_width((width-320.0).max(350.0));card(ui,|ui|{let rect=photo(ui,editor.texture.as_ref(),ui.available_width());if !end{
            if editor.record.edit.stats{ui.painter().text(rect.left_top()+Vec2::new(10.0,12.0),egui::Align2::LEFT_TOP,format!("作業 {}分 · 無操作 {}分 · {}枚",editor.record.active_ms.saturating_sub(editor.record.idle_ms)/60000,editor.record.idle_ms/60000,editor.record.frames),egui::FontId::proportional(13.0),Color32::WHITE);}
            if editor.position<2.5&&editor.record.edit.theme!="clean"{ui.painter().text(rect.center_top()+Vec2::new(0.0,rect.height()*0.15),egui::Align2::CENTER_CENTER,&editor.record.title,egui::FontId::proportional(18.0),Color32::WHITE);}
            if let Ok(captions)=parse_captions(&editor.captions){let text=captions.iter().filter(|caption|editor.position>=caption.start&&editor.position<caption.start+caption.duration).map(|caption|caption.text.as_str()).collect::<Vec<_>>().join("\n");ui.painter().text(rect.center_bottom()-Vec2::new(0.0,rect.height()*0.12),egui::Align2::CENTER_CENTER,text,egui::FontId::proportional(18.0),Color32::WHITE);}
        }ui.horizontal(|ui|{if ui.button(if editor.playing.is_some(){"Ⅱ 停止"}else{"▶ プレビュー"}).clicked(){if editor.playing.is_some(){editor.playing=None;editor.player=None;}else{if editor.position>=total{editor.position=0.0;}editor.playing=Some((Instant::now(),editor.position));}}
            if ui.add(egui::Slider::new(&mut editor.position,0.0..=total).suffix(" 秒")).changed(){editor.playing=None;editor.player=None;}});ui.small(format!("{}枚 → {}枚 / {:.1}秒（エンドカード3秒を含む）",editor.record.frames,editor.indices.len(),total));ui.small("プレビューは映像とテロップ。BGM・フェード・音声の最終確認は生成したMP4で。");});
            ui.add_space(10.0);card(ui,|ui|{ui.heading("テロップ");ui.add(egui::TextEdit::multiline(&mut editor.captions).desired_rows(5).desired_width(f32::INFINITY).hint_text("0:00 | 作業スタート\n0:05-0:10 | 下描きを進める"));ui.small("動画上の時刻 | 内容。終了時刻を省略すると5秒間表示します。");});
            ui.add_space(10.0);card(ui,|ui|{ui.heading("自分の声でアフレコ");ui.add_enabled_ui(snapshot.export.is_none(),|ui|{ui.horizontal(|ui|{
                if let Some(mic)=&editor.microphone{ui.label(format!("● 録音中 {}秒",mic.started.elapsed().as_secs()));if ui.button("■ 停止して保存").clicked(){let result=self.finish_voice(&mut editor);self.result(result);}}
                else if ui.button("● マイク録音を開始").clicked(){let result=(||{self.engine.editable(&editor.record.id)?;let directory=tempfile::tempdir()?;let mic=Microphone::start(&directory.path().join("voice.wav"))?;editor.voice_temp=Some(directory);editor.microphone=Some(mic);Ok(())})();self.result(result);}
                if editor.record.narration_path.is_some()&&editor.microphone.is_none(){if ui.button("音声を再生").clicked(){let result=(||{let name=PathBuf::from(editor.record.narration_path.as_ref().unwrap());let file=self.engine.store.directory(&editor.record.id)?.join(name.file_name().context("録音がありません。")?);let actual=if file.extension().is_some_and(|extension|extension=="wav"){file}else{let temporary=tempfile::tempdir()?;let decoded=temporary.path().join("preview.wav");export::decode_voice(&file,&decoded)?;editor.decoded_voice=Some(temporary);decoded};let player=Player::open(&actual)?;player.sink.set_volume(editor.record.edit.narration_volume);editor.player=Some(player);Ok(())})();self.result(result);}if ui.button("音声を停止").clicked(){editor.player=None;}if ui.button("録音を削除").clicked(){let result=(||{self.engine.editable(&editor.record.id)?;editor.player=None;if let Some(path)=editor.record.narration_path.take(){let name=PathBuf::from(path);fs::remove_file(self.engine.store.directory(&editor.record.id)?.join(name.file_name().context("録音がありません。")?))?;}self.engine.save_edit(&editor.record)?;Ok(())})();self.result(result);}}
            });ui.label("開始時刻（動画の秒）");ui.add(egui::DragValue::new(&mut editor.record.edit.narration_offset).range(0.0..=3600.0).speed(0.1));ui.add(egui::Slider::new(&mut editor.record.edit.narration_volume,0.0..=2.0).text("声の音量"));ui.small("1回10分・100MBまで。停止時に保存します。");});});
        });ui.vertical(|ui|{ui.set_width(280.0);card(ui,|ui|{string_choice(ui,"theme","テーマ演出",&mut editor.record.edit.theme,&[("paper","Paper · お絵描き"),("terminal","Terminal · 開発"),("cinema","Cinema"),("clean","Clean")]);string_choice(ui,"bgm","BGMの自動付与",&mut editor.record.edit.bgm,&[("auto","制作の種類に合わせる"),("calm","Calm · 穏やかな和音"),("focus","Focus · 集中するリズム"),("custom","自分の音楽ファイル"),("none","BGMなし")]);
            ui.add_enabled_ui(snapshot.export.is_none()&&editor.microphone.is_none(),|ui|{if ui.button("音楽ファイルを選ぶ").clicked(){if let Some(file)=rfd::FileDialog::new().add_filter("音楽",&["mp3","wav","m4a","ogg","flac"]).pick_file(){let result=(||{anyhow::ensure!(fs::metadata(&file)?.len()<=300*1024*1024,"BGMは300MBまでです。");let extension=file.extension().unwrap().to_string_lossy();let destination=self.engine.store.directory(&editor.record.id)?.join(format!("bgm.{extension}"));let mut temporary=tempfile::NamedTempFile::new_in(destination.parent().unwrap())?;std::io::copy(&mut fs::File::open(&file)?,temporary.as_file_mut())?;temporary.persist(&destination)?;editor.record.bgm_path=Some(destination.to_string_lossy().into_owned());editor.record.bgm_name=file.file_name().map(|name|name.to_string_lossy().into_owned());editor.record.edit.bgm="custom".into();self.engine.save_edit(&editor.record)?;Ok(())})();self.result(result);}}});
            ui.small(editor.record.bgm_name.as_deref().unwrap_or("内蔵BGMをローカルで生成します。"));ui.add(egui::Slider::new(&mut editor.record.edit.bgm_volume,0.0..=1.0).text("BGM音量"));ui.checkbox(&mut editor.record.edit.stats,"作業スタッツを重ね表示");let speed=editor.record.idle_speed;choice(ui,"editor-idle","無操作の追加早送り",&mut editor.record.idle_speed,&[(1,"なし"),(4,"4倍"),(8,"8倍"),(16,"16倍")]);if speed!=editor.record.idle_speed{let result=self.engine.store.timeline(&editor.record.id);if let Some(timeline)=self.result(result){editor.indices=selected_frames(&editor.record,&timeline);editor.playing=None;editor.position=0.0;}}
            ui.separator();ui.label(RichText::new("FREE EDITION").color(GREEN));ui.label("最後にTickreelを3秒表示");ui.add_enabled_ui(snapshot.export.is_none()&&editor.microphone.is_none(),|ui|{if ui.button("編集内容を保存").clicked(){let result=self.save_editor(&mut editor);if self.result(result).is_some(){self.message="編集内容を保存しました。".into();}}if primary(ui,"✦ ワンクリック生成").clicked(){let result=(||{self.save_editor(&mut editor)?;self.engine.export(vec![editor.record.clone()],self.engine.quick_destination(&editor.record.title),editor.record.edit.clone())})();self.result(result);}if ui.button("保存先を指定して生成").clicked(){if let Some(file)=rfd::FileDialog::new().add_filter("MP4",&["mp4"]).set_file_name("Tickreel.mp4").save_file(){let result=(||{self.save_editor(&mut editor)?;self.engine.export(vec![editor.record.clone()],file,editor.record.edit.clone())})();self.result(result);}}});
        });});});
        if !close {
            self.editor = Some(editor);
        }
    }
}
impl eframe::App for Desktop {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let snapshot = self.engine.snapshot();
        if self.refresh.elapsed() > Duration::from_secs(1) {
            let result = self
                .engine
                .store
                .import_obs()
                .and_then(|_| self.engine.store.list());
            if let Some(records) = self.result(result) {
                self.records = records;
            }
            self.refresh = Instant::now();
        }
        if let Ok(result) = self.import.try_recv() {
            if let Some(count) = self.result(result) {
                if count > 0 {
                    self.message = format!(
                        "旧版から{count}件の記録を取り込みました。旧版のデータは残しています。"
                    );
                }
            }
        }
        if let Some(error) = &snapshot.error {
            self.message = error.clone();
        }
        if snapshot.message != self.last_engine_message {
            if let Some(message) = &snapshot.message {
                self.message = message.clone();
            }
            self.last_engine_message = snapshot.message.clone();
        }
        if snapshot.revision != self.revision {
            self.revision = snapshot.revision;
            if let Some(image) = &snapshot.preview {
                self.texture = Some(texture(ctx, image, "capture"));
            }
        }
        if ctx.input(|input| input.viewport().close_requested()) && !self.allow_close {
            if snapshot.export.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.message = "動画の生成が完了するまでお待ちください。".into();
            } else if snapshot.current.is_some()
                || self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.microphone.is_some())
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_confirm = true;
            }
        }
        egui::SidePanel::left("navigation")
            .exact_width(205.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(30.0);
                ui.heading(RichText::new("tickreel.").color(GREEN).size(28.0));
                ui.small("RUST NATIVE · FREE EDITION");
                ui.add_space(45.0);
                if ui
                    .selectable_label(matches!(self.page, Page::Today), "◉ 今日の記録")
                    .clicked()
                {
                    self.page = Page::Today;
                }
                if ui
                    .selectable_label(
                        matches!(self.page, Page::Library),
                        format!("▤ ライブラリ  {}", self.records.len()),
                    )
                    .clicked()
                {
                    self.page = Page::Library;
                }
                ui.add_space(35.0);
                ui.label("すべてPC内に保存");
                ui.small("画面・音声を外部に送信しません。");
                if ui.button("保存フォルダーを開く").clicked() {
                    native::open(&self.engine.store.root, false);
                }
                ui.separator();
                let signature = crate::app_links::signature();
                if ui
                    .link(format!("Made by {} ↗", signature.creator.name))
                    .clicked()
                {
                    let result = signature.open("creator");
                    self.result(result);
                }
                if signature.creator.works_url.is_some() && ui.link("私が作ったもの ↗").clicked()
                {
                    let result = signature.open("works");
                    self.result(result);
                }
                for link in &signature.links {
                    if ui.link(format!("{} ↗", link.label)).clicked() {
                        let result = signature.open(&link.id);
                        self.result(result);
                    }
                }
                ui.small(format!("v{}", env!("CARGO_PKG_VERSION")));
            });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            if let Some(job) = &snapshot.export {
                ui.horizontal(|ui| {
                    ui.label(format!("{}を生成中", job.title));
                    ui.add(
                        egui::ProgressBar::new(job.percent as f32 / 100.0)
                            .desired_width(250.0)
                            .show_percentage(),
                    );
                });
            }
            if !self.message.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(&self.message);
                    if ui.small_button("×").clicked() {
                        self.message.clear();
                    }
                });
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("main").show(ui, |ui| {
                ui.add_space(20.0);
                if self.editor.is_some() {
                    self.editor(ui, ctx, &snapshot);
                } else {
                    match self.page {
                        Page::Today => self.today(ui, ctx, &snapshot),
                        Page::Library => self.library(ui, ctx, &snapshot),
                    }
                }
            });
        });
        if let Some(id) = self.delete_id.clone() {
            egui::Window::new("記録を削除しますか？")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("フレーム・テロップ・アフレコを削除します。元に戻せません。");
                    ui.label("生成済みのMP4は残ります。");
                    ui.horizontal(|ui| {
                        if ui.button("キャンセル").clicked() {
                            self.delete_id = None;
                        }
                        if ui.button("削除する").clicked() {
                            let result = self.engine.delete(&id);
                            if self.result(result).is_some() {
                                self.message = "記録を削除しました。".into();
                                self.delete_id = None;
                                self.refresh = Instant::now() - Duration::from_secs(2);
                            }
                        }
                    });
                });
        }
        if self.close_confirm {
            egui::Window::new("記録を保存して終了しますか？")
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("画面の記録とマイク録音を停止して保存します。");
                    ui.horizontal(|ui| {
                        if ui.button("続ける").clicked() {
                            self.close_confirm = false;
                        }
                        if ui
                            .add_enabled(
                                snapshot.export.is_none(),
                                egui::Button::new("保存して終了"),
                            )
                            .clicked()
                        {
                            let result = (|| {
                                if let Some(mut editor) = self.editor.take() {
                                    self.finish_voice(&mut editor)?;
                                }
                                self.engine.stop()?;
                                Ok(())
                            })();
                            if self.result(result).is_some() {
                                self.allow_close = true;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    });
                });
        }
        if let Some(count) = &mut self.qa_frames {
            *count += 1;
            if *count == 15 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            if *count > 30 {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        for event in ctx.input(|input| input.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                let bytes: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                if let Some(picture) =
                    image::RgbaImage::from_raw(image.size[0] as u32, image.size[1] as u32, bytes)
                {
                    let _ = fs::create_dir_all("artifacts");
                    let _ =
                        picture.save(if std::env::args().any(|arg| arg == "--ui-test-editor") {
                            "artifacts/rust-editor.png"
                        } else {
                            "artifacts/rust-ui.png"
                        });
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}
