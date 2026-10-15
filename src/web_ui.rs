use crate::{
    audio::{self, Microphone},
    engine::Engine,
    model::{Edit, Recording, period_records, selected_frames},
    native,
};
use anyhow::{Context, Result};
use base64::Engine as _;
use image::{Rgb, RgbImage, codecs::jpeg::JpegEncoder};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};
use wry::{
    PageLoadEvent, WebContext, WebViewBuilder,
    http::{Request, Response},
};

#[derive(Clone, Deserialize)]
struct Rpc {
    id: u64,
    method: String,
    #[serde(default)]
    args: Vec<Value>,
}
#[derive(Clone)]
enum Message {
    Request(Rpc),
    Reply(u64, Value, Option<String>),
    Ready,
    QaReport(Value),
    Captured(bool),
}
struct Backend {
    engine: Engine,
    qa: bool,
    mic_public: Mutex<Option<Value>>,
    export_ids: Mutex<Vec<String>>,
    closing: AtomicBool,
}
struct ActiveMic {
    id: String,
    temporary: tempfile::TempDir,
    microphone: Option<Microphone>,
    started: Instant,
}
#[derive(Default)]
struct Microphones {
    active: Option<ActiveMic>,
}
impl Microphones {
    fn start(&mut self, backend: &Backend, id: &str) -> Result<Value> {
        anyhow::ensure!(self.active.is_none(), "すでにマイクで録音しています。");
        backend.engine.editable(id)?;
        backend.engine.store.get(id)?;
        let temporary = tempfile::tempdir()?;
        let microphone = if backend.qa {
            audio::make_music(&temporary.path().join("voice.wav"), false)?;
            None
        } else {
            Some(Microphone::start(&temporary.path().join("voice.wav"))?)
        };
        let meta = json!({"id":id,"active":true,"startedAt":chrono::Utc::now().timestamp_millis()});
        self.active = Some(ActiveMic {
            id: id.into(),
            temporary,
            microphone,
            started: Instant::now(),
        });
        *backend.mic_public.lock().unwrap() = Some(meta.clone());
        Ok(meta)
    }
    fn stop(&mut self, backend: &Backend, id: &str) -> Result<Value> {
        if let Some(active) = &mut self.active {
            anyhow::ensure!(active.id == id, "録音している記録が異なります。");
            if let Some(microphone) = active.microphone.take() {
                microphone.finish()?;
            }
            let destination = backend.engine.store.directory(id)?.join("narration.wav");
            audio::commit_audio(&active.temporary.path().join("voice.wav"), &destination)?;
            let mut record = backend.engine.store.get(id)?;
            record.narration_path = Some(destination.to_string_lossy().into_owned());
            backend.engine.save_edit(&record)?;
            self.active = None;
            *backend.mic_public.lock().unwrap() = None;
            return Ok(serde_json::to_value(record)?);
        }
        Ok(serde_json::to_value(backend.engine.store.get(id)?)?)
    }
    fn reached_limit(&self) -> bool {
        self.active.as_ref().is_some_and(|active| {
            active.started.elapsed().as_secs() >= 600
                || active
                    .microphone
                    .as_ref()
                    .is_some_and(Microphone::should_stop)
        })
    }
}
fn data_image(image: &RgbImage, width: u32) -> Result<String> {
    let scaled = image::imageops::thumbnail(image, width, width * 9 / 16);
    let mut bytes = vec![];
    JpegEncoder::new_with_quality(&mut bytes, 85).encode_image(&scaled)?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}
fn arg<'a>(request: &'a Rpc, index: usize) -> Result<&'a Value> {
    request
        .args
        .get(index)
        .context("操作の引数を確認してください。")
}
fn id(request: &Rpc) -> Result<&str> {
    arg(request, 0)?
        .as_str()
        .context("記録IDを確認してください。")
}
impl Backend {
    fn not_recording_mic(&self) -> Result<()> {
        anyhow::ensure!(
            self.mic_public.lock().unwrap().is_none(),
            "マイク録音を停止してから操作してください。"
        );
        Ok(())
    }
    fn destination(&self, title: &str) -> PathBuf {
        if self.qa {
            self.engine
                .store
                .root
                .parent()
                .unwrap()
                .join(format!("{}.mp4", uuid::Uuid::new_v4()))
        } else {
            self.engine.quick_destination(title)
        }
    }
    fn export_records(&self, records: Vec<Recording>, options: &Value) -> Result<Value> {
        self.not_recording_mic()?;
        let first = records
            .first()
            .context("この期間に保存済みの記録がありません。")?;
        let title = options
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or(&first.title);
        let format = options
            .get("format")
            .and_then(Value::as_str)
            .unwrap_or("mp4")
            .to_ascii_lowercase();
        crate::export::output_format(Path::new(&format!("output.{format}")))?;
        let destination = if options.get("quick").and_then(Value::as_bool) != Some(false) || self.qa
        {
            self.destination(title).with_extension(&format)
        } else {
            let default = self.engine.quick_destination(title).with_extension(&format);
            let selection = rfd::FileDialog::new()
                .add_filter(format.to_uppercase(), &[format.as_str()])
                .set_file_name(default.file_name().unwrap().to_string_lossy())
                .save_file();
            let Some(selection) = selection else {
                return Ok(Value::Null);
            };
            selection.with_extension(&format)
        };
        let edit = if let Some(edit) = options.get("edit") {
            serde_json::from_value(edit.clone())?
        } else {
            first.edit.clone()
        };
        let ids: Vec<_> = records.iter().map(|record| record.id.clone()).collect();
        self.engine
            .export_with_reveal(records, destination.clone(), edit, !self.qa)?;
        *self.export_ids.lock().unwrap() = ids.clone();
        loop {
            if self.engine.snapshot().export.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let snapshot = self.engine.snapshot();
        anyhow::ensure!(
            snapshot.message.is_some(),
            "{}",
            snapshot
                .error
                .unwrap_or_else(|| "動画生成に失敗しました。".into())
        );
        *self.export_ids.lock().unwrap() = vec![];
        Ok(json!({"path":destination.to_string_lossy(),"recordings":ids.len()}))
    }
    fn rpc(&self, request: &Rpc) -> Result<Value> {
        anyhow::ensure!(
            !self.closing.load(Ordering::Relaxed),
            "アプリを終了しています。"
        );
        match request.method.as_str() {
            "state" => {
                let snapshot = self.engine.snapshot();
                let records = self.engine.store.list()?;
                let export_ids = self.export_ids.lock().unwrap().clone();
                Ok(
                    json!({"current":snapshot.current,"recordings":records,"exportState":snapshot.export.map(|job|json!({"id":export_ids.first(),"ids":export_ids,"progress":job.percent})),"captureError":snapshot.error,"root":self.engine.store.root.to_string_lossy(),"micState":*self.mic_public.lock().unwrap()}),
                )
            }
            "windows" => {
                if self.qa {
                    return Ok(json!([{"process":"TestApp","title":"プライバシーテスト"}]));
                }
                Ok(json!(
                    native::windows()?
                        .into_iter()
                        .filter(|window| !window.title.is_empty())
                        .map(|window| json!({"process":window.process,"title":window.title}))
                        .collect::<Vec<_>>()
                ))
            }
            "sources" => {
                let protected: Vec<String> = request
                    .args
                    .first()
                    .map(|value| serde_json::from_value(value.clone()))
                    .transpose()?
                    .unwrap_or_default();
                anyhow::ensure!(protected.len() <= 50, "黒塗り対象は50アプリまでです。");
                let windows = if protected.is_empty() || self.qa {
                    vec![]
                } else {
                    native::windows()?
                };
                let monitors = native::monitors()?;
                let mut output = vec![];
                for (index, monitor) in monitors.iter().enumerate() {
                    let mut image = if self.qa {
                        RgbImage::from_pixel(480, 270, Rgb([228, 239, 235]))
                    } else {
                        native::capture(monitor)?
                    };
                    if !self.qa {
                        let mut bounds = windows.clone();
                        if !protected.is_empty() {
                            bounds.extend(native::windows()?);
                        }
                        native::redact(&mut image, monitor.bounds, &bounds, &protected);
                    }
                    output.push(json!({"id":monitor.id,"name":if self.qa{format!("テスト画面 {}",index+1)}else{monitor.name.clone()},"thumbnail":data_image(&image,480)?}));
                }
                Ok(json!(output))
            }
            "start" => {
                let mut record: Recording = serde_json::from_value(arg(request, 0)?.clone())?;
                record.id = uuid::Uuid::new_v4().to_string();
                record.frames = 0;
                record.active_ms = 0;
                record.idle_ms = 0;
                record.bytes = 0;
                record.status = "recording".into();
                record.started_at = chrono::Utc::now().to_rfc3339();
                record.ended_at = None;
                record.narration_path = None;
                record.bgm_path = None;
                anyhow::ensure!(
                    record.protected_apps.len() <= 50
                        && record.protected_apps.iter().all(|name| name.len() <= 100),
                    "黒塗り対象を確認してください。"
                );
                self.engine.start(record)?;
                Ok(json!(self.engine.snapshot().current))
            }
            "pause" => {
                self.engine.pause()?;
                Ok(Value::Null)
            }
            "resume" => {
                self.engine.resume()?;
                Ok(Value::Null)
            }
            "stop" => {
                self.engine.stop()?;
                Ok(Value::Null)
            }
            "plan" => {
                let mut record = self.engine.store.get(id(request)?)?;
                if let Some(speed) = request.args.get(1).and_then(Value::as_u64) {
                    if [1, 4, 8, 16].contains(&speed) {
                        record.idle_speed = speed as u32;
                    }
                }
                let indices = selected_frames(&record, &self.engine.store.timeline(&record.id)?);
                let duration = indices.len() as f64 / record.fps as f64;
                Ok(json!({"indices":indices,"duration":duration,"totalDuration":duration+3.0}))
            }
            "preview" => {
                let requested = id(request)?;
                let snapshot = self.engine.snapshot();
                if snapshot
                    .current
                    .as_ref()
                    .is_some_and(|record| record.id == requested)
                {
                    return snapshot
                        .preview
                        .as_ref()
                        .map(|image| data_image(image, 960).map(Value::String))
                        .transpose()
                        .map(|value| value.unwrap_or(Value::Null));
                }
                let record = self.engine.store.get(requested)?;
                if record.frames == 0 {
                    return Ok(Value::Null);
                }
                let index = request.args.get(1).and_then(Value::as_u64).unwrap_or(0);
                anyhow::ensure!(index < record.frames as u64, "フレームが見つかりません。");
                let image = image::open(
                    self.engine
                        .store
                        .directory(requested)?
                        .join(format!("frame-{index:07}.jpg")),
                )?
                .to_rgb8();
                Ok(Value::String(data_image(&image, 960)?))
            }
            "edit" => {
                self.not_recording_mic()?;
                let mut record = self.engine.store.get(id(request)?)?;
                record.edit = serde_json::from_value(arg(request, 1)?.clone())?;
                if let Some(speed) = arg(request, 1)?.get("idleSpeed").and_then(Value::as_u64) {
                    if [1, 4, 8, 16].contains(&speed) {
                        record.idle_speed = speed as u32;
                    }
                }
                self.engine.save_edit(&record)?;
                Ok(json!(record))
            }
            "delete" => {
                self.not_recording_mic()?;
                let requested = id(request)?;
                self.engine.editable(requested)?;
                let record = self.engine.store.get(requested)?;
                let confirmed=self.qa||matches!(rfd::MessageDialog::new().set_title("記録の削除").set_description(format!("「{}」を削除しますか？\nフレーム・テロップ・録音を削除します。元に戻せません。生成済みMP4は残します。",record.title)).set_buttons(rfd::MessageButtons::YesNo).set_level(rfd::MessageLevel::Warning).show(),rfd::MessageDialogResult::Yes);
                if confirmed {
                    self.engine.delete(requested)?;
                }
                Ok(json!(confirmed))
            }
            "export" => {
                let record = self.engine.store.get(id(request)?)?;
                self.export_records(vec![record], request.args.get(1).unwrap_or(&Value::Null))
            }
            "summary" => {
                let options = arg(request, 0)?;
                let date = chrono::NaiveDate::parse_from_str(
                    options.get("date").and_then(Value::as_str).unwrap_or(""),
                    "%Y-%m-%d",
                )?;
                let period = options.get("period").and_then(Value::as_str).unwrap_or("");
                anyhow::ensure!(
                    ["week", "month"].contains(&period),
                    "期間を指定してください。"
                );
                let records = period_records(&self.engine.store.list()?, date, period == "month");
                let edit = Edit {
                    theme: options
                        .get("theme")
                        .and_then(Value::as_str)
                        .unwrap_or("cinema")
                        .into(),
                    ..Default::default()
                };
                self.export_records(records,&json!({"quick":true,"format":options.get("format").and_then(Value::as_str).unwrap_or("mp4"),"title":format!("{}まとめ-{date}",if period=="month"{"月間"}else{"週間"}),"edit":edit}))
            }
            "folder" => {
                if !self.qa {
                    native::open(&self.engine.store.root, false);
                }
                Ok(Value::Null)
            }
            "audioPreview" => {
                let requested = id(request)?;
                let record = self.engine.store.get(requested)?;
                Ok(record
                    .narration_path
                    .map(|_| json!(format!("http://tickreel.localhost/audio/{requested}")))
                    .unwrap_or(Value::Null))
            }
            "removeAudio" => {
                self.not_recording_mic()?;
                let requested = id(request)?;
                self.engine.editable(requested)?;
                anyhow::ensure!(
                    arg(request, 1)?.as_str() == Some("narration"),
                    "音声の種類を確認してください。"
                );
                let mut record = self.engine.store.get(requested)?;
                if let Some(path) = record.narration_path.take() {
                    let basename = Path::new(&path)
                        .file_name()
                        .context("音声の保存先がありません。")?;
                    let file = self.engine.store.directory(requested)?.join(basename);
                    if file.is_file() {
                        fs::remove_file(file)?;
                    }
                }
                self.engine.save_edit(&record)?;
                Ok(json!(record))
            }
            "pickBgm" => {
                self.not_recording_mic()?;
                let requested = id(request)?;
                self.engine.editable(requested)?;
                let file = if self.qa {
                    let file = self
                        .engine
                        .store
                        .directory(requested)?
                        .join("sample-bgm.wav");
                    audio::make_music(&file, false)?;
                    file
                } else {
                    let Some(file) = rfd::FileDialog::new()
                        .add_filter("音楽", &["wav", "mp3", "m4a", "ogg", "flac"])
                        .pick_file()
                    else {
                        return Ok(Value::Null);
                    };
                    file
                };
                anyhow::ensure!(
                    fs::metadata(&file)?.len() <= 300 * 1024 * 1024,
                    "BGMは300MBまでです。"
                );
                let destination = self.engine.store.directory(requested)?.join(format!(
                    "bgm.{}",
                    file.extension()
                        .context("音楽ファイルを選んでください。")?
                        .to_string_lossy()
                ));
                let mut staged = tempfile::NamedTempFile::new_in(destination.parent().unwrap())?;
                std::io::copy(&mut fs::File::open(&file)?, staged.as_file_mut())?;
                staged.persist(&destination)?;
                let mut record = self.engine.store.get(requested)?;
                record.bgm_path = Some(destination.to_string_lossy().into_owned());
                record.bgm_name = file
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned());
                record.edit.bgm = "custom".into();
                self.engine.save_edit(&record)?;
                Ok(json!(record))
            }
            _ => anyhow::bail!("未対応の操作です。"),
        }
    }
    fn asset(&self, request: &Request<Vec<u8>>) -> Result<Response<Cow<'static, [u8]>>> {
        let path = request.uri().path();
        let (mime, body) = match path {
            "/" | "/index.html" => (
                "text/html; charset=utf-8",
                Cow::Borrowed(include_bytes!("../frontend/index.html").as_slice()),
            ),
            "/style.css" => (
                "text/css; charset=utf-8",
                Cow::Borrowed(include_bytes!("../frontend/style.css").as_slice()),
            ),
            "/bridge.js" => (
                "text/javascript; charset=utf-8",
                Cow::Borrowed(include_bytes!("../frontend/bridge.js").as_slice()),
            ),
            "/renderer.js" => (
                "text/javascript; charset=utf-8",
                Cow::Borrowed(include_bytes!("../frontend/renderer.js").as_slice()),
            ),
            _ if path.starts_with("/audio/") => {
                let requested = &path[7..];
                let record = self.engine.store.get(requested)?;
                let source = record.narration_path.context("録音がありません。")?;
                let basename = Path::new(&source)
                    .file_name()
                    .context("音声の保存先がありません。")?;
                let file = self.engine.store.directory(requested)?.join(basename);
                let mime = if file.extension().is_some_and(|extension| extension == "wav") {
                    "audio/wav"
                } else {
                    "audio/webm"
                };
                let mut input = fs::File::open(file)?;
                let length = input.metadata()?.len();
                if let Some(range) = request
                    .headers()
                    .get("range")
                    .and_then(|range| range.to_str().ok())
                    .and_then(|range| range.strip_prefix("bytes="))
                {
                    let (start, end) = range.split_once('-').context("範囲を確認してください。")?;
                    let start: u64 = start.parse()?;
                    let end: u64 = if end.is_empty() {
                        length.saturating_sub(1)
                    } else {
                        end.parse()?
                    };
                    let end = end.min(length.saturating_sub(1));
                    anyhow::ensure!(
                        start <= end && end < length,
                        "音声の範囲を確認してください。"
                    );
                    input.seek(SeekFrom::Start(start))?;
                    let mut bytes = vec![];
                    input.take(end - start + 1).read_to_end(&mut bytes)?;
                    return Ok(Response::builder()
                        .status(206)
                        .header("Content-Type", mime)
                        .header("Accept-Ranges", "bytes")
                        .header("Content-Range", format!("bytes {start}-{end}/{length}"))
                        .body(Cow::Owned(bytes))?);
                }
                let mut bytes = vec![];
                input.read_to_end(&mut bytes)?;
                (mime, Cow::Owned(bytes))
            }
            _ => {
                return Ok(Response::builder()
                    .status(404)
                    .body(Cow::Borrowed(b"Not found".as_slice()))?);
            }
        };
        Ok(Response::builder()
            .header("Content-Type", mime)
            .header("Cache-Control", "no-store")
            .header("Accept-Ranges", "bytes")
            .body(body)?)
    }
}

pub fn run() -> Result<()> {
    anyhow::ensure!(
        wry::webview_version().is_ok(),
        "WebView2 Runtimeが必要です。WindowsのWebView2 Runtimeをインストールしてください。"
    );
    let qa = std::env::args().any(|arg| arg.starts_with("--ui-test"));
    let qa_root = if qa { Some(tempfile::tempdir()?) } else { None };
    let root = if let Some(temporary) = &qa_root {
        temporary.path().join("records")
    } else {
        dirs::data_dir()
            .context("データフォルダーがありません。")?
            .join("TickreelRust/recordings")
    };
    let engine = if qa {
        Engine::with_capture(root, |_| {
            Ok(RgbImage::from_pixel(1920, 1080, Rgb([226, 237, 232])))
        })?
    } else {
        Engine::new(root)?
    };
    let backend = Arc::new(Backend {
        engine,
        qa,
        mic_public: Mutex::new(None),
        export_ids: Mutex::new(vec![]),
        closing: AtomicBool::new(false),
    });
    if !qa {
        let store = backend.engine.store.clone();
        let legacy = dirs::data_dir().unwrap().join("tickreel/recordings");
        std::thread::spawn(move || {
            if let Err(error) = store.import_legacy(&legacy) {
                eprintln!("{error:#}");
            }
        });
    }
    let mut event_loop = EventLoopBuilder::<Message>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("Tickreel")
        .with_inner_size(LogicalSize::new(1320.0, 900.0))
        .with_min_inner_size(LogicalSize::new(1040.0, 760.0))
        .build(&event_loop)?;
    let mut context = WebContext::new(Some(
        backend.engine.store.root.parent().unwrap().join("WebView2"),
    ));
    let asset_backend = backend.clone();
    let request_proxy = proxy.clone();
    let load_proxy = proxy.clone();
    let webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_asynchronous_custom_protocol("tickreel".into(), move |_, request, responder| {
            let backend = asset_backend.clone();
            std::thread::spawn(move || {
                let response = backend.asset(&request).unwrap_or_else(|_| {
                    Response::builder()
                        .status(404)
                        .body(Cow::Borrowed(b"Not found".as_slice()))
                        .unwrap()
                });
                responder.respond(response);
            });
        })
        .with_ipc_handler(move |request| {
            let uri = request.uri().to_string();
            if !(uri.starts_with("http://tickreel.localhost/")
                || uri.starts_with("tickreel://localhost/"))
                || request.body().len() > 1024 * 1024
            {
                return;
            }
            if let Ok(request) = serde_json::from_str::<Rpc>(request.body()) {
                let _ = request_proxy.send_event(Message::Request(request));
            }
        })
        .with_navigation_handler(|url| {
            url.starts_with("http://tickreel.localhost/")
                || url.starts_with("tickreel://localhost/")
        })
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = load_proxy.send_event(Message::Ready);
            }
        })
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
        .with_background_color((246, 247, 249, 255))
        .with_url(if std::env::args().any(|arg| arg == "--ui-test-editor") {
            "tickreel://localhost/#editor"
        } else {
            "tickreel://localhost/"
        })
        .build(&window)?;
    let mut microphones = Microphones::default();
    let mut qa_deadline: Option<Instant> = None;
    let mut qa_failed = false;
    let mut qa_started = false;
    let opened = Instant::now();
    event_loop.run_return(|event,_,flow|{
        *flow=ControlFlow::WaitUntil(Instant::now()+Duration::from_millis(200));
        match event {
            Event::UserEvent(Message::Request(request))=>{
                if request.method=="frontendError"{if qa{qa_failed=true;eprintln!("Frontend: {}",request.args.first().unwrap_or(&Value::Null));}return;}
                if request.method=="qaReport"&&qa{let _=proxy.send_event(Message::QaReport(request.args.first().cloned().unwrap_or(Value::Null)));return;}
                if ["micStart","micStop"].contains(&request.method.as_str()){
                    let result=(||{let requested=id(&request)?;if request.method=="micStart"{microphones.start(&backend,requested)}else{microphones.stop(&backend,requested)}})();let(result,error)=match result{Ok(value)=>(value,None),Err(error)=>(Value::Null,Some(format!("{error:#}")))};let _=proxy.send_event(Message::Reply(request.id,result,error));
                }else{let backend=backend.clone();let proxy=proxy.clone();std::thread::spawn(move||{let result=backend.rpc(&request);let(result,error)=match result{Ok(value)=>(value,None),Err(error)=>(Value::Null,Some(format!("{error:#}")))};let _=proxy.send_event(Message::Reply(request.id,result,error));});}
            },
            Event::UserEvent(Message::Reply(id,result,error))=>{let script=format!("window.__tickreelResponse({id},{},{})",result,serde_json::to_string(&error).unwrap());if let Err(error)=webview.evaluate_script(&script){eprintln!("{error}");}},
            Event::UserEvent(Message::Ready)=>{if qa&&!qa_started{qa_started=true;let _=webview.evaluate_script(include_str!("../frontend/qa.js"));}},
            Event::UserEvent(Message::Captured(success))=>{qa_failed|=!success;*flow=ControlFlow::Exit;},
            Event::UserEvent(Message::QaReport(result))=>{let _=fs::create_dir_all("artifacts");let _=fs::write("artifacts/restored-ui-tests.json",serde_json::to_vec_pretty(&result).unwrap());if result.get("success")!=Some(&Value::Bool(true)){qa_failed=true;eprintln!("UI test: {result}");}qa_deadline=Some(Instant::now()+Duration::from_millis(800));},
            Event::WindowEvent{event:WindowEvent::CloseRequested,..}=>{
                if backend.engine.snapshot().export.is_some(){rfd::MessageDialog::new().set_title("Tickreel").set_description("動画の生成が完了してから閉じてください。").show();return;}
                if backend.engine.snapshot().current.is_some()||microphones.active.is_some(){let confirmed=matches!(rfd::MessageDialog::new().set_title("Tickreel").set_description("記録・マイク録音を停止して保存し、終了しますか？").set_buttons(rfd::MessageButtons::YesNo).show(),rfd::MessageDialogResult::Yes);if !confirmed{return;}if let Some(active)=&microphones.active{let id=active.id.clone();if let Err(error)=microphones.stop(&backend,&id){eprintln!("{error:#}");return;}}if backend.engine.stop().is_err(){return;}}
                backend.closing.store(true,Ordering::Relaxed);*flow=ControlFlow::Exit;
            },
            Event::MainEventsCleared=>{
                if qa&&opened.elapsed()>Duration::from_secs(240){qa_failed=true;eprintln!("UI test timed out");*flow=ControlFlow::Exit;return;}
                if microphones.reached_limit(){let id=microphones.active.as_ref().unwrap().id.clone();let result=microphones.stop(&backend,&id);let detail=match result{Ok(record)=>json!({"record":record}),Err(error)=>json!({"error":error.to_string()})};let _=webview.evaluate_script(&format!("window.dispatchEvent(new CustomEvent('tickreel-native-mic-stopped',{{detail:{detail}}}))"));}
                if qa_deadline.is_some_and(|deadline|Instant::now()>=deadline){qa_deadline=None;let file=if std::env::args().any(|arg|arg=="--ui-test-editor"){"artifacts/restored-editor.png"}else{"artifacts/restored-ui.png"};if let Err(error)=capture_webview(&webview,proxy.clone(),PathBuf::from(file)){eprintln!("Screenshot: {error:#}");qa_failed=true;*flow=ControlFlow::Exit;}}
            },_=>{},
        }
    });
    drop(webview);
    drop(backend);
    drop(context);
    drop(qa_root);
    anyhow::ensure!(!qa_failed, "元のUIとRust処理の接続テストに失敗しました。");
    Ok(())
}
fn capture_webview(
    webview: &wry::WebView,
    proxy: tao::event_loop::EventLoopProxy<Message>,
    file: PathBuf,
) -> Result<()> {
    use webview2_com::{
        CapturePreviewCompletedHandler,
        Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
    };
    use windows::Win32::System::Com::{
        STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET, StructuredStorage::CreateStreamOnHGlobal,
    };
    use wry::WebViewExtWindows;
    let stream =
        unsafe { CreateStreamOnHGlobal(windows::Win32::Foundation::HGLOBAL::default(), true)? };
    let captured = stream.clone();
    let callback = CapturePreviewCompletedHandler::create(Box::new(move |result| {
        let outcome = (|| -> Result<()> {
            result?;
            let mut stat: STATSTG = unsafe { std::mem::zeroed() };
            unsafe {
                captured.Stat(&mut stat, STATFLAG_NONAME)?;
                captured.Seek(0, STREAM_SEEK_SET, None)?;
            }
            anyhow::ensure!(
                stat.cbSize > 0 && stat.cbSize < 32 * 1024 * 1024,
                "描画画像のサイズを確認してください。"
            );
            let mut bytes = vec![0u8; stat.cbSize as usize];
            let mut read = 0;
            unsafe {
                captured
                    .Read(
                        bytes.as_mut_ptr().cast(),
                        bytes.len() as u32,
                        Some(&mut read),
                    )
                    .ok()?;
            }
            bytes.truncate(read as usize);
            image::load_from_memory(&bytes)?;
            fs::write(file, bytes)?;
            Ok(())
        })();
        if let Err(error) = &outcome {
            eprintln!("Screenshot: {error:#}");
        }
        let _ = proxy.send_event(Message::Captured(outcome.is_ok()));
        Ok(())
    }));
    unsafe {
        webview.controller().CoreWebView2()?.CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &stream,
            &callback,
        )?;
    }
    Ok(())
}
