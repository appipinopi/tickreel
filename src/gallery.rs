//! Separate application process: closing the gallery never closes a recording.
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};
use wry::{PageLoadEvent, WebContext, WebViewBuilder};

#[derive(Clone, Deserialize)]
struct Request {
    id: u64,
    method: String,
    #[serde(default)]
    args: Vec<Value>,
}
#[derive(Clone)]
enum Message {
    Request(Request),
    Reply(u64, Value, Option<String>),
    Ready,
}

pub fn run(qa: bool) -> Result<()> {
    let mut events = EventLoopBuilder::<Message>::with_user_event().build();
    let proxy = events.create_proxy();
    let png = image::load_from_memory(include_bytes!("../assets/tickreel.png"))?.into_rgba8();
    let (width, height) = png.dimensions();
    let icon = tao::window::Icon::from_rgba(png.into_raw(), width, height)?;
    let window = WindowBuilder::new()
        .with_title("私が作ったもの · appipinopi")
        .with_window_icon(Some(icon))
        .with_inner_size(LogicalSize::new(780.0, 580.0))
        .with_min_inner_size(LogicalSize::new(430.0, 420.0))
        .build(&events)?;
    let mut context = WebContext::new(None);
    let request_proxy = proxy.clone();
    let ready_proxy = proxy.clone();
    let builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_asynchronous_custom_protocol("appgallery".into(), move |_, request, responder| {
            let html = request.uri().path() == "/";
            let response = wry::http::Response::builder()
                .status(if html { 200 } else { 404 })
                .header("Content-Type", "text/html; charset=utf-8")
                .body(std::borrow::Cow::Borrowed(if html {
                    app_signature::GALLERY_HTML.as_bytes()
                } else {
                    b"Not found"
                }))
                .unwrap();
            responder.respond(response);
        })
        .with_initialization_script(&format!("window.__galleryQa={qa};"))
        .with_ipc_handler(move |request| {
            if request.body().len() > 32768 {
                return;
            }
            if let Ok(request) = serde_json::from_str::<Request>(request.body()) {
                let _ = request_proxy.send_event(Message::Request(request));
            }
        })
        .with_navigation_handler(|url| {
            url == "appgallery://localhost/" || url == "http://appgallery.localhost/"
        })
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = ready_proxy.send_event(Message::Ready);
            }
        })
        .with_url("appgallery://localhost/");
    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window)?;
    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        builder.build_gtk(window.default_vbox().context("GTK window is unavailable")?)?
    };
    let opened = Instant::now();
    let mut failed = false;
    let mut report_received = false;
    events.run_return(|event, _, flow| {
        *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(150));
        match event {
            Event::UserEvent(Message::Request(request)) => {
                if request.method == "qaReport" && qa {
                    let report = request.args.first().cloned().unwrap_or(Value::Null);
                    report_received = true;
                    failed = report.get("success") != Some(&Value::Bool(true));
                    let _ = std::fs::create_dir_all("artifacts");
                    let _ = std::fs::write(
                        "artifacts/gallery-ui-tests.json",
                        serde_json::to_vec_pretty(&report).unwrap(),
                    );
                    *flow = ControlFlow::Exit;
                    return;
                }
                let proxy = proxy.clone();
                std::thread::spawn(move || {
                    let result = (|| -> Result<Value> {
                        let signature = crate::app_links::signature();
                        match request.method.as_str() {
                            "catalog" => Ok(json!(signature)),
                            "openApp" => {
                                let id = request
                                    .args
                                    .first()
                                    .and_then(Value::as_str)
                                    .context("アプリを選択してください。")?;
                                let key = format!("app:{id}");
                                let url = signature.resolve(&key)?.to_owned();
                                if !qa {
                                    signature.open(&key)?;
                                }
                                Ok(json!({"id":id,"url":url}))
                            }
                            _ => anyhow::bail!("Unknown gallery command"),
                        }
                    })();
                    let (value, error) = match result {
                        Ok(value) => (value, None),
                        Err(error) => (Value::Null, Some(error.to_string())),
                    };
                    let _ = proxy.send_event(Message::Reply(request.id, value, error));
                });
            }
            Event::UserEvent(Message::Reply(id, value, error)) => {
                let _ = webview.evaluate_script(&format!(
                    "window.__galleryReply({id},{value},{})",
                    serde_json::to_string(&error).unwrap()
                ));
            }
            Event::UserEvent(Message::Ready) => {}
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *flow = ControlFlow::Exit,
            Event::MainEventsCleared if qa && opened.elapsed() > Duration::from_secs(30) => {
                failed = true;
                *flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
    drop(webview);
    drop(context);
    anyhow::ensure!(
        !failed && (!qa || report_received),
        "アプリ一覧の表示テストに失敗しました。"
    );
    Ok(())
}
