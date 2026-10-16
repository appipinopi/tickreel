//! ScreenCast portal authorization and bounded, in-memory PipeWire previews.
//! No X11 or compositor-specific screenshot API is used on Wayland.
use crate::{
    native::{Bounds, Monitor},
    wayland_frame::{self, PixelFormat},
};
use anyhow::{Context, Result, ensure};
use ashpd::desktop::{
    PersistMode,
    screencast::{CursorMode, Screencast, SourceType},
};
use futures_util::StreamExt;
use image::RgbImage;
use std::{
    io::Cursor,
    os::fd::{BorrowedFd, OwnedFd},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

static CAPTURE: Mutex<Option<Capture>> = Mutex::new(None);
const STREAM_ERROR: &str = "画面共有が停止しました。「共有する画面を選ぶ」から選び直してください。";

#[derive(Default)]
struct Frame {
    image: Option<Arc<RgbImage>>,
    streaming: bool,
    error: Option<String>,
}

#[derive(Default)]
struct Frames {
    streams: Vec<Frame>,
    error: Option<String>,
}

type Shared = Arc<(Mutex<Frames>, Condvar)>;

struct PortalReady {
    fd: OwnedFd,
    nodes: Vec<u32>,
}

struct Capture {
    id: String,
    shared: Shared,
    stopped: Arc<AtomicBool>,
    stop_portal: Option<oneshot::Sender<()>>,
    portal: Option<JoinHandle<()>>,
    pipewire: Option<JoinHandle<()>>,
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Some(stop) = self.stop_portal.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.pipewire.take() {
            let _ = worker.join();
        }
        if let Some(worker) = self.portal.take() {
            let _ = worker.join();
        }
    }
}

fn fail(shared: &Shared, message: impl Into<String>) {
    shared.0.lock().unwrap().error = Some(message.into());
    shared.1.notify_all();
}

fn portal(
    shared: Shared,
    stopped: Arc<AtomicBool>,
    mut stop: oneshot::Receiver<()>,
    ready: mpsc::SyncSender<Result<PortalReady>>,
) {
    let result = (|| -> Result<()> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            let proxy = tokio::time::timeout(Duration::from_secs(15), Screencast::new()).await??;
            let session = tokio::time::timeout(Duration::from_secs(15), proxy.create_session()).await??;
            let mut closed = session.receive_closed().await?;
            let operation = async {
                let modes = proxy.available_cursor_modes().await?;
                let cursor = if modes.contains(CursorMode::Embedded) { CursorMode::Embedded } else { CursorMode::Hidden };
                proxy.select_sources(&session, cursor, SourceType::Monitor.into(), true, None, PersistMode::DoNot)
                    .await?.response()?;
                let response = proxy.start(&session, None).await?.response()?;
                ensure!(!response.streams().is_empty() && response.streams().len() <= 8, "共有する画面を1〜8枚選んでください。");
                let nodes = response.streams().iter().map(|s| s.pipe_wire_node_id()).collect::<Vec<_>>();
                let fd = proxy.open_pipe_wire_remote(&session).await?;
                shared.0.lock().unwrap().streams = (0..nodes.len()).map(|_| Frame::default()).collect();
                ready.send(Ok(PortalReady { fd, nodes })).map_err(|_| anyhow::anyhow!("画面共有の初期化を終了しました。"))?;
                Ok(())
            };
            let result: Result<bool> = tokio::select! {
                result = tokio::time::timeout(Duration::from_secs(180), operation) => {
                    match result {
                        Ok(result) => result.map(|()| true),
                        Err(error) => Err(anyhow::anyhow!("画面選択がタイムアウトしました。{error}")),
                    }
                },
                _ = &mut stop => Ok(false),
            };
            let result = match result {
                Ok(true) => tokio::select! {
                    _ = closed.next() => Err(anyhow::anyhow!(STREAM_ERROR)),
                    _ = &mut stop => Ok(()),
                },
                Ok(false) => Ok(()),
                Err(error) => Err(error),
            };
            let _ = tokio::time::timeout(Duration::from_secs(3), session.close()).await;
            result
        })
    })();
    if let Err(error) = result {
        let message = format!(
            "Waylandの画面共有を開始・継続できませんでした。画面選択を許可し、PipeWireとxdg-desktop-portal（GNOME/KDE/wlr等）の起動を確認してください。\n{error:#}"
        );
        fail(&shared, &message);
        let _ = ready.send(Err(anyhow::anyhow!(message)));
    }
    stopped.store(true, Ordering::Release);
}

impl Capture {
    fn new() -> Result<Self> {
        let shared = Arc::new((Mutex::new(Frames::default()), Condvar::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let (stop_portal, stop) = oneshot::channel();
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut capture = Self {
            id: uuid::Uuid::new_v4().to_string(),
            shared: shared.clone(),
            stopped: stopped.clone(),
            stop_portal: Some(stop_portal),
            portal: None,
            pipewire: None,
        };
        capture.portal = Some(thread::spawn(move || portal(shared, stopped, stop, sender)));
        let ready = receiver
            .recv_timeout(Duration::from_secs(195))
            .context("画面共有の応答がありません。")??;
        let shared = capture.shared.clone();
        let stopped = capture.stopped.clone();
        capture.pipewire = Some(thread::spawn(move || {
            if let Err(error) = pipewire_capture(ready, shared.clone(), stopped.clone()) {
                fail(
                    &shared,
                    format!("PipeWireの画面取得に失敗しました。\n{error:#}"),
                );
                stopped.store(true, Ordering::Release);
            }
        }));
        // Dimensions come from negotiated pixel buffers, not optional portal
        // logical sizes (which differ under fractional scaling).
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut state = capture.shared.0.lock().unwrap();
        loop {
            ensure!(
                state.error.is_none(),
                "{}",
                state.error.as_deref().unwrap_or(STREAM_ERROR)
            );
            if !state.streams.is_empty()
                && state
                    .streams
                    .iter()
                    .all(|s| s.image.is_some() && s.streaming)
            {
                break;
            }
            ensure!(
                Instant::now() < deadline,
                "共有画面の映像を取得できませんでした。ロックを解除し、画面を選び直してください。"
            );
            state = capture
                .shared
                .1
                .wait_timeout(state, Duration::from_millis(100))
                .unwrap()
                .0;
        }
        drop(state);
        Ok(capture)
    }

    fn monitors(&self) -> Result<Vec<Monitor>> {
        let state = self.shared.0.lock().unwrap();
        ensure!(
            !self.stopped.load(Ordering::Acquire) && state.error.is_none(),
            "{}",
            state.error.as_deref().unwrap_or(STREAM_ERROR)
        );
        Ok(state
            .streams
            .iter()
            .enumerate()
            .filter_map(|(index, stream)| {
                let image = stream.image.as_ref()?;
                Some(Monitor {
                    id: format!("wayland:{}:{index}", self.id),
                    name: format!("共有画面 {}", index + 1),
                    bounds: Bounds {
                        x: 0,
                        y: 0,
                        width: image.width(),
                        height: image.height(),
                    },
                })
            })
            .collect())
    }
}

pub fn select() -> Result<()> {
    let mut capture = CAPTURE.lock().unwrap();
    // Keep the existing authorized session if the chooser is cancelled.
    let replacement = Capture::new()?;
    *capture = Some(replacement);
    Ok(())
}

pub fn release() {
    CAPTURE.lock().unwrap().take();
}

pub fn monitors() -> Result<Vec<Monitor>> {
    CAPTURE
        .lock()
        .unwrap()
        .as_ref()
        .map_or(Ok(vec![]), Capture::monitors)
}

pub fn capture(monitor: &Monitor) -> Result<RgbImage> {
    let capture = CAPTURE.lock().unwrap();
    let capture = capture.as_ref().context(STREAM_ERROR)?;
    let prefix = format!("wayland:{}:", capture.id);
    let index: usize = monitor
        .id
        .strip_prefix(&prefix)
        .context(STREAM_ERROR)?
        .parse()?;
    let state = capture.shared.0.lock().unwrap();
    ensure!(
        !capture.stopped.load(Ordering::Acquire) && state.error.is_none(),
        "{}",
        state.error.as_deref().unwrap_or(STREAM_ERROR)
    );
    let stream = state.streams.get(index).context(STREAM_ERROR)?;
    ensure!(
        stream.streaming && stream.error.is_none(),
        "{}",
        stream.error.as_deref().unwrap_or(
            "共有画面の映像が一時停止しています。ロック解除後に記録を再開してください。"
        )
    );
    Ok(stream
        .image
        .as_ref()
        .context("共有画面の映像が届いていません。")?
        .as_ref()
        .clone())
}

pub fn unlocked() -> bool {
    // logind covers compositor locks even if a compositor retains the last
    // PipeWire buffer. Stream state is checked again at the point of capture.
    let locked = (|| -> Result<bool> {
        let connection = zbus::blocking::Connection::system()?;
        let manager = zbus::blocking::Proxy::new(
            &connection,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )?;
        let path: zbus::zvariant::OwnedObjectPath =
            manager.call("GetSessionByPID", &(std::process::id(),))?;
        let session = zbus::blocking::Proxy::new(
            &connection,
            "org.freedesktop.login1",
            path,
            "org.freedesktop.login1.Session",
        )?;
        Ok(session.get_property::<bool>("LockedHint")?)
    })()
    .unwrap_or(false);
    !locked
}

pub fn self_test() -> Result<()> {
    ensure!(
        crate::capture_platform::is_wayland(),
        "Waylandセッションで実行してください。"
    );
    ensure!(
        monitors()?.is_empty(),
        "起動前に画面共有が開始されています。"
    );
    let result = (|| -> Result<()> {
        select()?;
        let mut first = monitors()?;
        ensure!(!first.is_empty(), "選択した画面がありません。");
        for monitor in &first {
            let image = capture(monitor)?;
            ensure!(
                image.width() > 0 && image.height() > 0,
                "PipeWireの画面が空です。"
            );
            // The CI compositor has an isolated solid-color background.
            if std::env::var_os("TICKREEL_HEADLESS_WAYLAND_TEST").is_some() {
                let pixel = image.get_pixel(10.min(image.width() - 1), 10.min(image.height() - 1));
                ensure!(
                    pixel
                        .0
                        .iter()
                        .zip([51, 102, 153])
                        .all(|(a, b)| (*a as i16 - b).abs() <= 2),
                    "PipeWireのRGB色変換が不正です。{pixel:?}"
                );
            }
        }
        thread::sleep(Duration::from_millis(300));
        ensure!(
            monitors()?[0].id == first[0].id,
            "画面共有セッションが再作成されています。"
        );
        capture(&first[0])?;
        let temporary = tempfile::tempdir()?;
        let engine = crate::engine::Engine::new(temporary.path().join("records"))?;
        engine.start(crate::model::Recording {
            source_ids: vec![first[0].id.clone()],
            interval: 1,
            fps: 15,
            // Wayland must disable automatic idle compression at the boundary.
            idle_speed: 8,
            ..Default::default()
        })?;
        let deadline = Instant::now() + Duration::from_secs(8);
        while engine
            .snapshot()
            .current
            .as_ref()
            .is_some_and(|record| record.frames < 2)
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(100));
        }
        let saved = engine
            .stop()?
            .context("Waylandの記録が保存されませんでした。")?;
        ensure!(
            saved.frames >= 2 && saved.idle_speed == 1 && saved.bytes > 0,
            "Waylandの記録・保存に失敗しました。"
        );
        drop(engine);
        let previous = first[0].clone();
        select()?;
        first = monitors()?;
        ensure!(
            first[0].id != previous.id && capture(&previous).is_err(),
            "選び直す前の画面IDが有効なままです。"
        );
        capture(&first[0])?;
        if std::env::var_os("TICKREEL_HEADLESS_WAYLAND_TEST").is_some() {
            let output = std::process::Command::new("swaymsg")
                .args(["output", "HEADLESS-1", "disable"])
                .output()?;
            ensure!(
                output.status.success(),
                "検証用ディスプレイを停止できませんでした。"
            );
            let deadline = Instant::now() + Duration::from_secs(5);
            while capture(&first[0]).is_ok() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(50));
            }
            ensure!(
                capture(&first[0]).is_err(),
                "共有停止後に古いフレームを取得できてしまいます。"
            );
        }
        release();
        ensure!(
            monitors()?.is_empty() && capture(&first[0]).is_err(),
            "共有終了後も画面を取得できてしまいます。"
        );
        Ok(())
    })();
    release();
    result
}

fn memfd_image(
    data: &pipewire::spa::buffer::Data,
    width: u32,
    height: u32,
    format: PixelFormat,
) -> Result<RgbImage> {
    use pipewire::spa::buffer::{ChunkFlags, DataType};
    ensure!(
        data.type_() == DataType::MemFd && data.fd() >= 0,
        "PipeWireの画面メモリ形式が未対応です。"
    );
    ensure!(
        !data.chunk().flags().contains(ChunkFlags::CORRUPTED),
        "PipeWireの画面データが破損しています。"
    );
    let length = data.as_raw().maxsize as usize;
    let map_offset = data.as_raw().mapoffset as u64;
    ensure!(
        length > 0 && length <= 256 * 1024 * 1024,
        "画面バッファーのサイズが不正です。"
    );
    // The fd is borrowed only while this dequeued buffer is owned by us.
    // Duplicate it so the mapping owns its lifetime independently of PipeWire.
    let fd = unsafe { BorrowedFd::borrow_raw(data.fd()) }.try_clone_to_owned()?;
    let file = std::fs::File::from(fd);
    ensure!(
        map_offset
            .checked_add(length as u64)
            .is_some_and(|end| end <= file.metadata().map(|m| m.len()).unwrap_or(0)),
        "画面メモリの長さが不足しています。"
    );
    // SAFETY: the validated fd is shared memory, not DMA-BUF. The producer
    // has handed this buffer to us until it is requeued; copy its pixels now.
    // Map read-only ourselves: some portal producers omit SPA_DATA_READABLE,
    // causing PW_STREAM_FLAG_MAP_BUFFERS to create an unreadable mapping.
    let map = unsafe {
        memmap2::MmapOptions::new()
            .offset(map_offset)
            .len(length)
            .map(&file)?
    };
    let offset = data.chunk().offset() as usize;
    let size = data.chunk().size() as usize;
    let valid_end = offset
        .checked_add(size)
        .filter(|end| *end <= map.len())
        .context("画面バッファーの範囲が不正です。")?;
    wayland_frame::decode(
        &map[..valid_end],
        width,
        height,
        offset,
        data.chunk().stride(),
        format,
    )
}

fn pipewire_capture(ready: PortalReady, shared: Shared, stopped: Arc<AtomicBool>) -> Result<()> {
    use pipewire::{
        context::ContextRc,
        main_loop::MainLoopRc,
        properties::properties,
        spa::{
            param::{
                ParamType,
                format::{FormatProperties, MediaSubtype, MediaType},
                video::{VideoFormat, VideoInfoRaw},
            },
            pod::{self, Pod, serialize::PodSerializer},
            utils::{Direction, Fraction, Rectangle, SpaTypes},
        },
        stream::{StreamFlags, StreamRc, StreamState},
    };
    pipewire::init();
    let main_loop = MainLoopRc::new(None)?;
    let context = ContextRc::new(&main_loop, None)?;
    // Only the remote FD granted by the portal can access the selected nodes.
    let core = context.connect_fd_rc(ready.fd, None)?;
    let mut streams = vec![];
    let mut listeners = vec![];
    for (index, node) in ready.nodes.into_iter().enumerate() {
        let stream = StreamRc::new(
            core.clone(),
            "Tickreel screen capture",
            properties! {
                "media.type" => "Video", "media.category" => "Capture", "media.role" => "Screen",
            },
        )?;
        let state_shared = shared.clone();
        let frame_shared = shared.clone();
        let format_shared = shared.clone();
        let listener = stream
            .add_local_listener_with_user_data(VideoInfoRaw::default())
            .state_changed(move |_, _, _, state| {
                let mut frames = state_shared.0.lock().unwrap();
                let frame = &mut frames.streams[index];
                frame.streaming = matches!(state, StreamState::Streaming);
                // Never retain a pre-lock frame across suspension/revocation.
                if !frame.streaming {
                    frame.image = None;
                }
                if let StreamState::Error(error) = state {
                    frame.error = Some(error.to_owned());
                }
                state_shared.1.notify_all();
            })
            .param_changed(move |stream, format, id, param| {
                if id != ParamType::Format.as_raw() {
                    return;
                }
                if let Some(param) = param {
                    if let Err(error) = format.parse(param) {
                        fail(
                            &format_shared,
                            format!("画面の形式を読み取れません。{error}"),
                        );
                        return;
                    }
                    let buffers = pod::Object {
                        type_: SpaTypes::ObjectParamBuffers.as_raw(),
                        id: ParamType::Buffers.as_raw(),
                        properties: vec![pod::Property::new(
                            pipewire::spa::sys::SPA_PARAM_BUFFERS_dataType,
                            pod::Value::Int(1 << pipewire::spa::buffer::DataType::MemFd.as_raw()),
                        )],
                    };
                    let result = (|| -> Result<()> {
                        let bytes = PodSerializer::serialize(
                            Cursor::new(Vec::new()),
                            &pod::Value::Object(buffers),
                        )?
                        .0
                        .into_inner();
                        stream
                            .update_params(&mut [Pod::from_bytes(&bytes)
                                .context("画面バッファーの形式が不正です。")?])?;
                        Ok(())
                    })();
                    if let Err(error) = result {
                        fail(
                            &format_shared,
                            format!("画面バッファーを設定できません。{error:#}"),
                        );
                    }
                }
            })
            .process(move |stream, format| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };
                let datas = buffer.datas_mut();
                let Some(data) = datas.first_mut() else {
                    return;
                };
                let size = data.chunk().size() as usize;
                if size == 0 {
                    return;
                }
                let pixel_format = match format.format() {
                    VideoFormat::RGB => PixelFormat::Rgb,
                    VideoFormat::BGR => PixelFormat::Bgr,
                    VideoFormat::RGBA | VideoFormat::RGBx => PixelFormat::Rgbx,
                    VideoFormat::BGRA | VideoFormat::BGRx => PixelFormat::Bgrx,
                    other => {
                        fail(&frame_shared, format!("未対応の画面形式です。{other:?}"));
                        return;
                    }
                };
                let dimensions = format.size();
                let decoded = memfd_image(data, dimensions.width, dimensions.height, pixel_format);
                let mut frames = frame_shared.0.lock().unwrap();
                match decoded {
                    Ok(image) => {
                        frames.streams[index].image = Some(Arc::new(image));
                    }
                    Err(error) => {
                        frames.streams[index].error = Some(format!("{error:#}"));
                        frames.streams[index].image = None;
                    }
                }
                frame_shared.1.notify_all();
            })
            .register()?;
        let object = pod::object!(
            SpaTypes::ObjectParamFormat,
            ParamType::EnumFormat,
            pod::property!(FormatProperties::MediaType, Id, MediaType::Video),
            pod::property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
            pod::property!(
                FormatProperties::VideoFormat,
                Choice,
                Enum,
                Id,
                VideoFormat::BGRx,
                VideoFormat::BGRx,
                VideoFormat::RGBx,
                VideoFormat::BGRA,
                VideoFormat::RGBA,
                VideoFormat::RGB,
                VideoFormat::BGR
            ),
            pod::property!(
                FormatProperties::VideoSize,
                Choice,
                Range,
                Rectangle,
                Rectangle {
                    width: 1920,
                    height: 1080
                },
                Rectangle {
                    width: 1,
                    height: 1
                },
                Rectangle {
                    width: 16384,
                    height: 16384
                }
            ),
            pod::property!(
                FormatProperties::VideoFramerate,
                Choice,
                Range,
                Fraction,
                Fraction { num: 5, denom: 1 },
                Fraction { num: 0, denom: 1 },
                Fraction { num: 60, denom: 1 }
            ),
        );
        let bytes = PodSerializer::serialize(Cursor::new(Vec::new()), &pod::Value::Object(object))?
            .0
            .into_inner();
        let mut params = [Pod::from_bytes(&bytes).context("PipeWireの画面形式が不正です。")?];
        stream.connect(
            Direction::Input,
            Some(node),
            StreamFlags::AUTOCONNECT,
            &mut params,
        )?;
        listeners.push(listener);
        streams.push(stream);
    }
    while !stopped.load(Ordering::Acquire) {
        main_loop
            .loop_()
            .iterate(pipewire::loop_::Timeout::Finite(Duration::from_millis(100)));
    }
    drop(listeners);
    drop(streams);
    Ok(())
}
