use crate::{
    audio, export,
    model::{Edit, FrameMeta, Recording, period_records, selected_frames},
    native,
    store::Store,
};
use anyhow::{Context, Result};
use chrono::{Local, Utc};
use fs2::FileExt;
use image::{Rgb, RgbImage};
use std::{
    fs::{self, File, OpenOptions},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

struct Session {
    record: Recording,
    segment: Option<Instant>,
    next: Instant,
    last_frame_ms: u64,
}
impl Session {
    fn elapsed(&self) -> u64 {
        self.record.active_ms
            + self
                .segment
                .map_or(0, |start| start.elapsed().as_millis() as u64)
    }
    fn snapshot(&self) -> Recording {
        let mut record = self.record.clone();
        record.active_ms = self.elapsed();
        record
    }
    fn pause(&mut self) {
        self.record.active_ms = self.elapsed();
        self.segment = None;
        self.record.status = "paused".into();
    }
}
#[derive(Clone)]
pub struct Job {
    pub percent: u32,
    pub title: String,
}
#[derive(Default)]
struct State {
    current: Option<Session>,
    preview: Option<Arc<RgbImage>>,
    revision: u64,
    error: Option<String>,
    export: Option<Job>,
    message: Option<String>,
}
#[derive(Clone)]
pub struct Snapshot {
    pub current: Option<Recording>,
    pub preview: Option<Arc<RgbImage>>,
    pub revision: u64,
    pub error: Option<String>,
    pub export: Option<Job>,
    pub message: Option<String>,
}
pub struct Engine {
    pub store: Store,
    state: Arc<Mutex<State>>,
    shutdown: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    _lock: File,
}
impl Engine {
    pub fn new(root: PathBuf) -> Result<Self> {
        Self::with_capture(root, capture_record)
    }
    pub(crate) fn with_capture(
        root: PathBuf,
        capture: fn(&Recording) -> Result<RgbImage>,
    ) -> Result<Self> {
        let store = Store::new(root)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(store.root.parent().unwrap().join("tickreel.lock"))?;
        lock.try_lock_exclusive()
            .context("Rust版Tickreelはすでに起動しています。既存のウィンドウを開いてください。")?;
        for mut record in store.list()? {
            if ["recording", "paused"].contains(&record.status.as_str()) {
                record.status = "interrupted".into();
                record.ended_at = Some(Utc::now().to_rfc3339());
                store.save(&record)?;
            }
        }
        let state = Arc::new(Mutex::new(State::default()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let state_clone = state.clone();
        let stop = shutdown.clone();
        let storage = store.clone();
        let worker = thread::spawn(move || {
            native::set_dpi();
            let mut last_tick = Instant::now();
            let mut awake = false;
            while !stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(150));
                let gap = last_tick.elapsed();
                last_tick = Instant::now();
                let task = {
                    let mut state = state_clone.lock().unwrap();
                    if let Some(session) = &mut state.current {
                        if session.record.status == "recording"
                            && (gap
                                > Duration::from_secs((session.record.interval as u64 * 2).max(20))
                                || !native::desktop_unlocked())
                        {
                            session.pause();
                            if let Err(error) = storage.save(&session.record) {
                                state.error = Some(error.to_string());
                            } else {
                                state.error = Some(
                                    "画面ロック・スリープを検出したため、一時停止しました。".into(),
                                );
                            }
                        }
                    }
                    let active = state
                        .current
                        .as_ref()
                        .is_some_and(|session| session.record.status == "recording");
                    if awake != active {
                        native::keep_awake(active);
                        awake = active;
                    }
                    state
                        .current
                        .as_ref()
                        .filter(|session| {
                            session.record.status == "recording" && session.next <= Instant::now()
                        })
                        .map(|session| session.record.clone())
                };
                if let Some(record) = task {
                    let captured = capture(&record);
                    let mut state = state_clone.lock().unwrap();
                    if !state.current.as_ref().is_some_and(|session| {
                        session.record.id == record.id && session.record.status == "recording"
                    }) {
                        continue;
                    }
                    match captured {
                        Ok(image) => {
                            let session = state.current.as_mut().unwrap();
                            let elapsed = session.elapsed();
                            let idle = native::idle_seconds() >= session.record.idle_threshold;
                            if idle {
                                session.record.idle_ms +=
                                    elapsed.saturating_sub(session.last_frame_ms);
                            }
                            session.last_frame_ms = elapsed;
                            let mut checkpoint = session.snapshot();
                            match storage.frame(&mut checkpoint, &image, idle) {
                                Ok(()) => {
                                    session.record.frames = checkpoint.frames;
                                    session.record.bytes = checkpoint.bytes;
                                    session.next = Instant::now()
                                        + Duration::from_secs(session.record.interval as u64);
                                    state.preview = Some(Arc::new(image));
                                    state.revision += 1;
                                    state.error = None;
                                }
                                Err(error) => {
                                    session.pause();
                                    let _ = storage.save(&session.record);
                                    state.error =
                                        Some(format!("保存に失敗し、一時停止しました: {error}"));
                                }
                            }
                        }
                        Err(error) => {
                            let session = state.current.as_mut().unwrap();
                            session.pause();
                            let _ = storage.save(&session.record);
                            state.error = Some(error.to_string());
                        }
                    }
                }
            }
            native::keep_awake(false);
        });
        Ok(Self {
            store,
            state,
            shutdown,
            worker: Some(worker),
            _lock: lock,
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        let state = self.state.lock().unwrap();
        Snapshot {
            current: state.current.as_ref().map(Session::snapshot),
            preview: state.preview.clone(),
            revision: state.revision,
            error: state.error.clone(),
            export: state.export.clone(),
            message: state.message.clone(),
        }
    }
    pub fn start(&self, mut record: Recording) -> Result<()> {
        record.validate()?;
        let monitors = native::monitors()?;
        anyhow::ensure!(
            record
                .source_ids
                .iter()
                .all(|id| monitors.iter().any(|monitor| &monitor.id == id)),
            "画面の接続を確認してください。"
        );
        if !record.protected_apps.is_empty() {
            native::windows()?;
        }
        record.source_id = record.source_ids[0].clone();
        record.source_name = record.source_ids.join(" + ");
        record.title = record.title.trim().chars().take(80).collect();
        if record.title.is_empty() {
            record.title = "今日の作業".into();
        }
        record.edit.theme = if record.category == "development" {
            "terminal"
        } else {
            "paper"
        }
        .into();
        let mut state = self.state.lock().unwrap();
        anyhow::ensure!(state.current.is_none(), "すでに記録中です。");
        self.store.save(&record)?;
        state.current = Some(Session {
            record,
            segment: Some(Instant::now()),
            next: Instant::now(),
            last_frame_ms: 0,
        });
        state.preview = None;
        state.error = None;
        Ok(())
    }
    pub fn pause(&self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if let Some(session) = &mut state.current {
            session.pause();
            self.store.save(&session.record)?;
        }
        Ok(())
    }
    pub fn resume(&self) -> Result<()> {
        anyhow::ensure!(
            native::desktop_unlocked(),
            "画面のロックを解除してください。"
        );
        let mut state = self.state.lock().unwrap();
        if let Some(session) = &mut state.current {
            if session.record.status == "paused" {
                session.record.status = "recording".into();
                session.last_frame_ms = session.record.active_ms;
                session.segment = Some(Instant::now());
                session.next = Instant::now();
                self.store.save(&session.record)?;
                state.error = None;
            }
        }
        Ok(())
    }
    pub fn stop(&self) -> Result<Option<Recording>> {
        let mut state = self.state.lock().unwrap();
        if let Some(mut session) = state.current.take() {
            session.pause();
            session.record.status = "complete".into();
            session.record.ended_at = Some(Utc::now().to_rfc3339());
            if let Err(error) = self.store.save(&session.record) {
                state.current = Some(session);
                return Err(error);
            }
            state.preview = None;
            Ok(Some(session.record))
        } else {
            Ok(None)
        }
    }
    pub fn editable(&self, id: &str) -> Result<()> {
        let snapshot = self.snapshot();
        anyhow::ensure!(
            snapshot.export.is_none() && !snapshot.current.is_some_and(|record| record.id == id),
            "記録・生成が終わってから変更してください。"
        );
        Ok(())
    }
    pub fn save_edit(&self, record: &Recording) -> Result<()> {
        self.editable(&record.id)?;
        anyhow::ensure!(
            record.edit.captions.len() <= 200,
            "テロップは200個までです。"
        );
        for caption in &record.edit.captions {
            anyhow::ensure!(
                caption.start.is_finite()
                    && caption.start >= 0.0
                    && caption.duration.is_finite()
                    && caption.duration > 0.0
                    && caption.duration <= 86400.0
                    && caption.x.is_finite()
                    && caption.y.is_finite()
                    && caption.size.is_finite()
                    && caption.text.chars().count() <= 250,
                "テロップの時間・文章・位置を確認してください。"
            );
        }
        self.store.save(record)
    }
    pub fn delete(&self, id: &str) -> Result<()> {
        self.editable(id)?;
        self.store.remove(id)
    }
    pub fn export(&self, records: Vec<Recording>, destination: PathBuf, edit: Edit) -> Result<()> {
        self.export_with_reveal(records, destination, edit, true)
    }
    pub fn export_with_reveal(
        &self,
        records: Vec<Recording>,
        destination: PathBuf,
        edit: Edit,
        reveal: bool,
    ) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        anyhow::ensure!(state.export.is_none(), "動画を生成中です。");
        anyhow::ensure!(
            !records.is_empty(),
            "この期間に保存済みの記録がありません。"
        );
        anyhow::ensure!(
            records.iter().all(|record| record.frames > 0
                && !state
                    .current
                    .as_ref()
                    .is_some_and(|session| session.record.id == record.id)),
            "記録を終了してから生成してください。"
        );
        state.export = Some(Job {
            percent: 0,
            title: if records.len() > 1 {
                format!("{}件のまとめ", records.len())
            } else {
                records[0].title.clone()
            },
        });
        state.message = None;
        let state_clone = self.state.clone();
        let store = self.store.clone();
        thread::spawn(move || {
            let result = export::generate(&store, &records, &destination, &edit, |percent| {
                if let Some(job) = &mut state_clone.lock().unwrap().export {
                    job.percent = percent;
                }
            });
            let mut state = state_clone.lock().unwrap();
            match result {
                Ok(result) => {
                    for mut record in records {
                        record.export_path = Some(result.path.to_string_lossy().into_owned());
                        record.exported_at = Some(Utc::now().to_rfc3339());
                        if let Err(error) = store.save(&record) {
                            state.error = Some(error.to_string());
                        }
                    }
                    state.message = Some(format!(
                        "ファイルを書き出しました（{:.1}秒）: {}",
                        result.duration,
                        result.path.display()
                    ));
                    if reveal {
                        native::open(&result.path, true);
                    }
                }
                Err(error) => state.error = Some(format!("{error:#}")),
            }
            state.export = None;
        });
        Ok(())
    }
    pub fn quick_destination(&self, title: &str) -> PathBuf {
        let safe: String = title
            .chars()
            .map(|c| {
                if "<>:\"/\\|?*".contains(c) || c.is_control() {
                    '_'
                } else {
                    c
                }
            })
            .take(60)
            .collect();
        dirs::video_dir()
            .unwrap_or_else(|| self.store.root.clone())
            .join("Tickreel")
            .join(format!(
                "{}-{}.mp4",
                if safe.is_empty() { "Tickreel" } else { &safe },
                Local::now().format("%Y-%m-%d-%H%M%S-%3f")
            ))
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.stop();
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn capture_record(record: &Recording) -> Result<RgbImage> {
    let mut windows = if record.protected_apps.is_empty() {
        vec![]
    } else {
        native::windows()?
    };
    let monitors = native::monitors()?;
    let mut frames = vec![];
    for id in &record.source_ids {
        let monitor = monitors
            .iter()
            .find(|monitor| &monitor.id == id)
            .context("記録する画面が切断されました。")?;
        frames.push((monitor.clone(), native::capture(monitor)?));
    }
    if !record.protected_apps.is_empty() {
        windows.extend(native::windows()?);
    }
    let mut images = vec![];
    for (monitor, mut image) in frames {
        native::redact(&mut image, monitor.bounds, &windows, &record.protected_apps);
        images.push(image);
    }
    native::compose(&images, record.layout == "horizontal")
}
pub fn self_test() -> Result<()> {
    let monitors = native::monitors()?;
    for monitor in &monitors {
        anyhow::ensure!(
            !native::capture(monitor)?.is_empty(),
            "画面取得に失敗しました。"
        );
    }
    native::windows()?;
    let root = tempfile::tempdir()?;
    let store = Store::new(root.path().join("records"))?;
    let mut secret = RgbImage::from_pixel(100, 100, Rgb([255, 255, 255]));
    native::redact(
        &mut secret,
        native::Bounds {
            x: -100,
            y: 0,
            width: 100,
            height: 100,
        },
        &[native::Window {
            process: "Private".into(),
            title: "secret".into(),
            bounds: native::Bounds {
                x: -60,
                y: 20,
                width: 40,
                height: 40,
            },
        }],
        &["Private".into()],
    );
    anyhow::ensure!(*secret.get_pixel(50, 40) == Rgb([0, 0, 0]), "黒塗り失敗");
    let image = native::compose(
        &[secret, RgbImage::from_pixel(100, 100, Rgb([185, 215, 199]))],
        false,
    )?;
    let mut record = Recording {
        title: "Rust版の検証".into(),
        category: "development".into(),
        source_ids: vec![monitors[0].id.clone()],
        active_ms: 3_600_000,
        idle_ms: 600_000,
        ..Default::default()
    };
    store.save(&record)?;
    for index in 0..45 {
        store.frame(&mut record, &image, index >= 25)?;
    }
    record.edit.captions = crate::model::parse_captions("0:00 | Rustで生成するテロップ")?;
    record.edit.theme = "terminal".into();
    let voice = store.directory(&record.id)?.join("narration.wav");
    audio::make_music(&voice, false)?;
    record.narration_path = Some(voice.to_string_lossy().into_owned());
    store.save(&record)?;
    let single = export::generate(
        &store,
        &[record.clone()],
        &root.path().join("single.mp4"),
        &record.edit,
        |_| {},
    )?;
    anyhow::ensure!(
        single.body_duration < 1.5 && single.duration == single.body_duration + 3.0,
        "早送り・エンドカード失敗"
    );
    for format in [
        "mov", "webm", "mkv", "avi", "gif", "wav", "mp3", "flac", "m4a", "ogg", "aac",
    ] {
        let result = export::generate(
            &store,
            &[record.clone()],
            &root.path().join(format!("format.{format}")),
            &record.edit,
            |_| {},
        )?;
        let probe = export::hidden_command(&export::ffmpeg())
            .arg("-i")
            .arg(&result.path)
            .args(["-f", "null", "-"])
            .output()?;
        anyhow::ensure!(
            probe.status.success(),
            "{format} 読み込み失敗: {}",
            String::from_utf8_lossy(&probe.stderr)
        );
        let audio_only = ["wav", "mp3", "flac", "m4a", "ogg", "aac"].contains(&format);
        anyhow::ensure!(
            !audio_only
                || (!String::from_utf8_lossy(&probe.stderr).contains("Video:")
                    && result.duration == result.body_duration),
            "音声出力に映像が混入しています。"
        );
    }
    let mut silent = record.edit.clone();
    silent.bgm = "none".into();
    let mut no_voice = record.clone();
    no_voice.narration_path = None;
    anyhow::ensure!(
        export::generate(
            &store,
            &[no_voice],
            &root.path().join("silent.wav"),
            &silent,
            |_| {}
        )
        .is_err(),
        "音声がない出力の検証失敗"
    );
    let mut second = record.clone();
    second.id = uuid::Uuid::new_v4().to_string();
    second.fps = 60;
    second.frames = 0;
    second.narration_path = None;
    store.save(&second)?;
    for _ in 0..6 {
        store.frame(&mut second, &image, false)?;
    }
    let combined = export::generate(
        &store,
        &[record.clone(), second],
        &root.path().join("combined.mp4"),
        &Edit {
            theme: "cinema".into(),
            bgm: "none".into(),
            ..Default::default()
        },
        |_| {},
    )?;
    anyhow::ensure!(
        (combined.body_duration - single.body_duration - 0.1).abs() < 0.001,
        "fps混在の結合失敗"
    );
    let probe = export::hidden_command(&export::ffmpeg())
        .args(["-hide_banner", "-i"])
        .arg(&single.path)
        .args(["-f", "null", "-"])
        .output()?;
    let stderr = String::from_utf8_lossy(&probe.stderr);
    anyhow::ensure!(
        probe.status.success() && stderr.contains("Audio: aac") && stderr.contains("1920x1080"),
        "動画・音声の検証失敗"
    );
    let last = root.path().join("end-frame.png");
    let output = export::hidden_command(&export::ffmpeg())
        .args(["-y", "-ss"])
        .arg((single.duration - 1.0).to_string())
        .arg("-i")
        .arg(&single.path)
        .args(["-frames:v", "1"])
        .arg(&last)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "エンドカードの読み込みに失敗しました。"
    );
    let pixel = *image::open(last)?.to_rgb8().get_pixel(100, 100);
    anyhow::ensure!(
        pixel[0] < 25 && pixel[1] > 35 && pixel[1] < 55,
        "エンドカードが動画に表示されていません。"
    );
    let imported = Store::new(root.path().join("imported"))?;
    anyhow::ensure!(
        imported.import_legacy(&store.root)? == 0,
        "記録中の記録を取り込んでしまいました。"
    );
    record.status = "complete".into();
    store.save(&record)?;
    anyhow::ensure!(
        imported.import_legacy(&store.root)? == 1,
        "旧版データの取り込み失敗"
    );
    anyhow::ensure!(
        period_records(&[record.clone()], Local::now().date_naive(), true).len() == 1,
        "期間集計失敗"
    );
    store.remove(&record.id)?;
    anyhow::ensure!(store.get(&record.id).is_err(), "削除失敗");
    let _ = (
        selected_frames(&record, &[FrameMeta::default()]),
        fs::metadata(single.path)?,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    static CAPTURE_STARTED: AtomicBool = AtomicBool::new(false);
    fn delayed_capture(_: &Recording) -> Result<RgbImage> {
        CAPTURE_STARTED.store(true, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(250));
        Ok(RgbImage::new(8, 8))
    }
    #[test]
    fn pause_discards_capture_that_is_already_running() {
        let root = tempfile::tempdir().unwrap();
        let engine = Engine::with_capture(root.path().join("records"), delayed_capture).unwrap();
        let monitor = native::monitors().unwrap().remove(0);
        let record = Recording {
            source_ids: vec![monitor.id],
            ..Default::default()
        };
        let id = record.id.clone();
        engine.start(record).unwrap();
        for _ in 0..100 {
            if CAPTURE_STARTED.load(Ordering::SeqCst) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(CAPTURE_STARTED.load(Ordering::SeqCst));
        engine.pause().unwrap();
        thread::sleep(Duration::from_millis(350));
        assert_eq!(engine.store.get(&id).unwrap().frames, 0);
        assert_eq!(engine.snapshot().current.unwrap().status, "paused");
        engine.stop().unwrap();
        assert_eq!(engine.store.get(&id).unwrap().status, "complete");
    }
}
