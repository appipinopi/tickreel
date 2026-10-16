#![allow(unsafe_op_in_unsafe_fn)]
#[path = "../../src/app_links.rs"]
mod app_links;
mod ffi;
#[path = "../../src/model.rs"]
#[allow(dead_code)]
mod model;
#[path = "../../src/store.rs"]
#[allow(dead_code)]
mod store;

use anyhow::{Context, Result, ensure};
use chrono::Utc;
use ffi::{Api, Ptr, ScaleInfo, SourceInfo, VideoData};
use fs2::FileExt;
use image::RgbImage;
use model::Recording;
use std::{
    ffi::{CStr, CString, c_char},
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use store::Store;

static API: OnceLock<Api> = OnceLock::new();
fn api() -> &'static Api {
    API.get().expect("OBS API initialized before callbacks")
}
fn recording_root() -> PathBuf {
    if let Some(root) = std::env::var_os("TICKREEL_OBS_RECORDINGS_DIR") {
        return PathBuf::from(root);
    }
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("TickreelRust/obs-recordings")
}

struct Capture {
    _lock: std::fs::File,
    sender: Option<SyncSender<RgbImage>>,
    worker: Option<JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
    running: Arc<AtomicBool>,
    next: Mutex<Instant>,
    interval: Duration,
    width: u32,
    height: u32,
}

impl Capture {
    fn new(
        root: PathBuf,
        title: String,
        interval: u32,
        fps: u32,
        width: u32,
        height: u32,
    ) -> Result<Box<Self>> {
        ensure!(
            [1, 5, 10, 30, 60].contains(&interval),
            "Interval must be 1, 5, 10, 30 or 60 seconds."
        );
        ensure!(
            [15, 24, 30, 60].contains(&fps),
            "FPS must be 15, 24, 30 or 60."
        );
        ensure!(
            width > 0 && height > 0 && width <= 16384 && height <= 16384,
            "Invalid OBS output dimensions."
        );
        let store = Store::new(root)?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(store.root.join("obs-capture.lock"))?;
        lock.try_lock_exclusive()
            .context("Another Tickreel OBS source is already recording.")?;
        for mut abandoned in store.list()? {
            if ["recording", "paused"].contains(&abandoned.status.as_str()) {
                abandoned.status = "interrupted".into();
                abandoned.ended_at = Some(Utc::now().to_rfc3339());
                store.save(&abandoned)?;
            }
        }
        let record = Recording {
            title,
            source_id: "obs-program".into(),
            source_ids: vec!["obs-program".into()],
            source_name: "OBS Program".into(),
            interval,
            fps,
            idle_speed: 1,
            ..Recording::default()
        };
        store.save(&record)?;
        let (sender, receiver) = mpsc::sync_channel::<RgbImage>(2);
        let error = Arc::new(Mutex::new(None));
        let failure = error.clone();
        let running = Arc::new(AtomicBool::new(true));
        let worker_running = running.clone();
        let worker = thread::Builder::new()
            .name("tickreel-obs-writer".into())
            .spawn(move || {
                let mut record = record;
                let started = Instant::now();
                let result = (|| -> Result<()> {
                    for image in receiver {
                        record.active_ms = started.elapsed().as_millis() as u64;
                        store.frame(&mut record, &image, false)?;
                    }
                    Ok(())
                })();
                worker_running.store(false, Ordering::Release);
                record.active_ms = started.elapsed().as_millis() as u64;
                record.ended_at = Some(Utc::now().to_rfc3339());
                record.status = if result.is_ok() {
                    "ready"
                } else {
                    "interrupted"
                }
                .into();
                if let Err(err) = result.and_then(|_| store.save(&record)) {
                    *failure.lock().unwrap() = Some(format!("Save failed: {err:#}"));
                    // Preserve metadata for the last successfully saved frame on disk-full errors.
                    let _ = store.save(&record);
                }
            })?;
        Ok(Box::new(Self {
            _lock: lock,
            sender: Some(sender),
            worker: Some(worker),
            error,
            running,
            next: Mutex::new(Instant::now()),
            interval: Duration::from_secs(interval as u64),
            width,
            height,
        }))
    }

    fn finish(&mut self) {
        self.running.store(false, Ordering::Release);
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.finish();
    }
}

// Copy on the video thread; bounded queue, JPEG encoding and disk I/O on the writer.
unsafe extern "C" fn video(data: Ptr, frame: *mut VideoData) {
    let capture = &*(data as *const Capture);
    if !capture.running.load(Ordering::Acquire) || frame.is_null() {
        return;
    }
    let Ok(mut next) = capture.next.try_lock() else {
        return;
    };
    let now = Instant::now();
    if now < *next {
        return;
    }
    *next = now + capture.interval;
    let frame = &*frame;
    let row = capture.width as usize * 4;
    if frame.data[0].is_null() || (frame.linesize[0] as usize) < row {
        return;
    }
    let length = (capture.height as usize - 1) * frame.linesize[0] as usize + row;
    let bytes = std::slice::from_raw_parts(frame.data[0], length);
    let image = bgra_to_rgb(
        bytes,
        capture.width,
        capture.height,
        frame.linesize[0] as usize,
    );
    if let Some(sender) = &capture.sender {
        // Dropping a frame is preferable to blocking OBS when the disk is slow.
        let _ = sender.try_send(image);
    }
}

fn bgra_to_rgb(bytes: &[u8], width: u32, height: u32, stride: usize) -> RgbImage {
    RgbImage::from_fn(width, height, |x, y| {
        let i = y as usize * stride + x as usize * 4;
        image::Rgb([bytes[i + 2], bytes[i + 1], bytes[i]])
    })
}

struct Controller {
    settings: Mutex<Settings>,
    capture: Mutex<Option<Box<Capture>>>,
    message: Mutex<String>,
}
struct Settings {
    title: String,
    interval: u32,
    fps: u32,
}
unsafe extern "C" fn name(_: Ptr) -> *const c_char {
    c"Tickreel Timelapse".as_ptr()
}
unsafe extern "C" fn create(settings: Ptr, _: Ptr) -> Ptr {
    let controller = Box::new(Controller {
        settings: Mutex::new(Settings {
            title: String::new(),
            interval: 1,
            fps: 15,
        }),
        capture: Mutex::new(None),
        message: Mutex::new("Ready / 待機中".into()),
    });
    let data = Box::into_raw(controller) as Ptr;
    update(data, settings);
    data
}
unsafe extern "C" fn update(data: Ptr, settings: Ptr) {
    let controller = &*(data as *const Controller);
    let raw = (api().obs_data_get_string)(settings, c"title".as_ptr());
    let title = if raw.is_null() {
        String::new()
    } else {
        CStr::from_ptr(raw).to_string_lossy().into_owned()
    };
    *controller.settings.lock().unwrap() = Settings {
        title: if title.trim().is_empty() {
            "OBS Timelapse".into()
        } else {
            title
        },
        interval: (api().obs_data_get_int)(settings, c"interval".as_ptr()).clamp(1, 60) as u32,
        fps: (api().obs_data_get_int)(settings, c"fps".as_ptr()).clamp(15, 60) as u32,
    };
}
unsafe fn stop(controller: &Controller) -> Option<String> {
    let capture = controller.capture.lock().unwrap().take();
    if let Some(mut capture) = capture {
        capture.running.store(false, Ordering::Release);
        (api().obs_remove_raw_video_callback)(video, (&mut *capture) as *mut Capture as Ptr);
        capture.finish();
        return capture.error.lock().unwrap().clone();
    }
    None
}
unsafe extern "C" fn destroy(data: Ptr) {
    if !data.is_null() {
        let controller = Box::from_raw(data as *mut Controller);
        stop(&controller);
    }
}
unsafe extern "C" fn defaults(settings: Ptr) {
    (api().obs_data_set_default_int)(settings, c"interval".as_ptr(), 1);
    (api().obs_data_set_default_int)(settings, c"fps".as_ptr(), 15);
    (api().obs_data_set_default_string)(settings, c"title".as_ptr(), c"OBS Timelapse".as_ptr());
}

unsafe fn status(properties: Ptr, text: &str) {
    let text = CString::new(text.replace('\0', "")).unwrap();
    let property = (api().obs_properties_get)(properties, c"status".as_ptr());
    (api().obs_property_set_description)(property, text.as_ptr());
}
unsafe fn controls(properties: Ptr, recording: bool) {
    for key in [c"title", c"interval", c"fps", c"start"] {
        (api().obs_property_set_enabled)(
            (api().obs_properties_get)(properties, key.as_ptr()),
            !recording,
        );
    }
    (api().obs_property_set_enabled)(
        (api().obs_properties_get)(properties, c"stop".as_ptr()),
        recording,
    );
}
unsafe extern "C" fn start_button(properties: Ptr, _: Ptr, data: Ptr) -> bool {
    if data.is_null() {
        return false;
    }
    let controller = &*(data as *const Controller);
    let mut slot = controller.capture.lock().unwrap();
    if slot.is_some() {
        return false;
    }
    let result = (|| -> Result<Box<Capture>> {
        let output = (api().obs_get_video)();
        ensure!(!output.is_null(), "OBS video is not initialized.");
        let settings = controller.settings.lock().unwrap();
        Capture::new(
            recording_root(),
            settings.title.clone(),
            settings.interval,
            settings.fps,
            (api().video_output_get_width)(output),
            (api().video_output_get_height)(output),
        )
    })();
    match result {
        Ok(mut capture) => {
            let conversion = ScaleInfo {
                format: 7,
                width: capture.width,
                height: capture.height,
                range: 2,
                colorspace: 3,
            };
            let ptr = (&mut *capture) as *mut Capture as Ptr;
            *slot = Some(capture);
            (api().obs_add_raw_video_callback)(&conversion, video, ptr);
            *controller.message.lock().unwrap() = "Recording OBS Program / 記録中".into();
            status(properties, &controller.message.lock().unwrap());
            controls(properties, true);
        }
        Err(error) => {
            *controller.message.lock().unwrap() = format!("Error: {error:#}");
            status(properties, &controller.message.lock().unwrap());
        }
    }
    true
}
unsafe extern "C" fn stop_button(properties: Ptr, _: Ptr, data: Ptr) -> bool {
    if data.is_null() {
        return false;
    }
    let controller = &*(data as *const Controller);
    let error = stop(controller);
    *controller.message.lock().unwrap() =
        error.unwrap_or_else(|| "Saved. Open Tickreel to edit and export. / 保存完了".into());
    status(properties, &controller.message.lock().unwrap());
    controls(properties, false);
    true
}
unsafe fn open_signature_link(properties: Ptr, data: Ptr, id: &str) -> bool {
    if let Err(error) = app_links::signature().open(id) {
        let message = format!("Browser error: {error:#}");
        if !data.is_null() {
            *(data as *const Controller)
                .as_ref()
                .unwrap()
                .message
                .lock()
                .unwrap() = message.clone();
        }
        status(properties, &message);
        return true;
    }
    false
}
unsafe extern "C" fn creator_button(properties: Ptr, _: Ptr, data: Ptr) -> bool {
    open_signature_link(properties, data, "creator")
}
unsafe extern "C" fn website_button(properties: Ptr, _: Ptr, data: Ptr) -> bool {
    open_signature_link(properties, data, "website")
}
unsafe extern "C" fn properties(data: Ptr) -> Ptr {
    let props = (api().obs_properties_create)();
    (api().obs_properties_add_text)(props, c"info".as_ptr(), c"Records the final OBS Program, including scene changes. Independent of OBS streaming/recording. / OBSの最終出力を記録します。".as_ptr(), 3);
    (api().obs_properties_add_text)(props, c"title".as_ptr(), c"Title / タイトル".as_ptr(), 0);
    for (key, label, values) in [
        (
            c"interval",
            c"Capture interval (seconds) / 撮影間隔（秒）",
            &[1i64, 5, 10, 30, 60][..],
        ),
        (
            c"fps",
            c"Export FPS / 書き出しfps",
            &[15i64, 24, 30, 60][..],
        ),
    ] {
        let list = (api().obs_properties_add_list)(props, key.as_ptr(), label.as_ptr(), 2, 1);
        for value in values {
            let label = CString::new(value.to_string()).unwrap();
            (api().obs_property_list_add_int)(list, label.as_ptr(), *value);
        }
    }
    let text = CString::new(format!("Save: {}", recording_root().display())).unwrap();
    (api().obs_properties_add_text)(props, c"path".as_ptr(), text.as_ptr(), 3);
    (api().obs_properties_add_text)(props, c"status".as_ptr(), c"Ready / 待機中".as_ptr(), 3);
    (api().obs_properties_add_button)(
        props,
        c"start".as_ptr(),
        c"Start timelapse / 記録を開始".as_ptr(),
        start_button,
    );
    (api().obs_properties_add_button)(
        props,
        c"stop".as_ptr(),
        c"Stop and save / 停止して保存".as_ptr(),
        stop_button,
    );
    let creator =
        CString::new(format!("Made by {} ↗", app_links::signature().creator.name)).unwrap();
    (api().obs_properties_add_button)(props, c"creator".as_ptr(), creator.as_ptr(), creator_button);
    (api().obs_properties_add_button)(
        props,
        c"website".as_ptr(),
        c"公式サイト ↗".as_ptr(),
        website_button,
    );
    let active = if data.is_null() {
        false
    } else {
        let controller = &*(data as *const Controller);
        status(props, &controller.message.lock().unwrap());
        let slot = controller.capture.lock().unwrap();
        if let Some(capture) = slot.as_ref() {
            if let Some(error) = capture.error.lock().unwrap().as_ref() {
                status(props, error);
            } else {
                status(props, "Recording OBS Program / 記録中");
            }
        }
        slot.is_some()
    };
    controls(props, active);
    props
}

#[unsafe(no_mangle)]
pub extern "C" fn obs_module_ver() -> u32 {
    31 << 24
}
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_set_pointer(_: Ptr) {}
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_name() -> *const c_char {
    c"Tickreel OBS".as_ptr()
}
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_description() -> *const c_char {
    c"Tickreel timelapse recorder 1.0.2".as_ptr()
}
#[unsafe(no_mangle)]
pub extern "C" fn obs_module_author() -> *const c_char {
    c"appipinopi".as_ptr()
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_load() -> bool {
    if API.get().is_none() {
        let Ok(loaded) = Api::load().context("Load libobs ABI") else {
            return false;
        };
        let _ = API.set(loaded);
    }
    let source = SourceInfo {
        id: c"tickreel_timelapse".as_ptr(),
        kind: 0,
        flags: 1 << 7,
        name,
        create,
        destroy,
        width: None,
        height: None,
        defaults,
        properties,
        update,
    };
    (api().obs_register_source_s)(&source, std::mem::size_of::<SourceInfo>());
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn padded_bgra_rows_preserve_colors() {
        let rgb = bgra_to_rgb(&[0, 0, 255, 255, 99, 99, 99, 99, 255, 0, 0, 255], 1, 2, 8);
        assert_eq!(rgb.get_pixel(0, 0).0, [255, 0, 0]);
        assert_eq!(rgb.get_pixel(0, 1).0, [0, 0, 255]);
    }
    #[test]
    fn capture_writes_compatible_completed_recording() {
        let dir = tempfile::tempdir().unwrap();
        let mut capture = Capture::new(dir.path().into(), "OBS test".into(), 1, 15, 2, 2).unwrap();
        capture
            .sender
            .as_ref()
            .unwrap()
            .send(RgbImage::new(2, 2))
            .unwrap();
        capture.finish();
        assert!(capture.error.lock().unwrap().is_none());
        let source = Store::new(dir.path().into()).unwrap();
        let records = source.list().unwrap();
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.frames, 1);
        assert_eq!(record.status, "ready");
        assert_eq!(record.idle_speed, 1);
        assert!(record.ended_at.is_some());
        assert!(
            source
                .directory(&record.id)
                .unwrap()
                .join("frame-0000000.jpg")
                .is_file()
        );
        assert_eq!(source.timeline(&record.id).unwrap().len(), 1);
        let imported = tempfile::tempdir().unwrap();
        let destination = Store::new(imported.path().into()).unwrap();
        assert_eq!(destination.import_legacy(dir.path()).unwrap(), 1);
        assert_eq!(destination.import_legacy(dir.path()).unwrap(), 0);
        assert_eq!(destination.list().unwrap()[0].frames, 1);
    }
    #[test]
    fn invalid_presets_do_not_create_recordings() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Capture::new(dir.path().into(), "bad".into(), 2, 15, 2, 2).is_err());
        assert!(Capture::new(dir.path().into(), "bad".into(), 1, 16, 2, 2).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
    #[test]
    fn single_writer_lock_and_abandoned_session_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().into()).unwrap();
        let mut abandoned = Recording::default();
        store.save(&abandoned).unwrap();
        store
            .frame(&mut abandoned, &RgbImage::new(2, 2), false)
            .unwrap();
        let first = Capture::new(dir.path().into(), "first".into(), 1, 15, 2, 2).unwrap();
        assert!(Capture::new(dir.path().into(), "second".into(), 1, 15, 2, 2).is_err());
        let recovered = store.get(&abandoned.id).unwrap();
        assert_eq!(recovered.status, "interrupted");
        assert!(recovered.ended_at.is_some());
        drop(first);
        assert!(Capture::new(dir.path().into(), "next".into(), 1, 15, 2, 2).is_ok());
    }
}
