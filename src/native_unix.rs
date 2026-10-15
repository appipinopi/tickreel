use anyhow::{Context, Result, ensure};
use image::{
    Rgb, RgbImage,
    imageops::{self, FilterType},
};

#[derive(Clone, Debug)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub bounds: Bounds,
}
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug)]
pub struct Window {
    pub process: String,
    pub title: String,
    pub bounds: Bounds,
}
pub fn set_dpi() {}
fn supported_session() -> Result<()> {
    #[cfg(target_os = "linux")]
    ensure!(
        std::env::var_os("DISPLAY").is_some()
            && std::env::var("XDG_SESSION_TYPE").unwrap_or_default() != "wayland",
        "Linux版の画面記録はX11セッションに対応しています。ログイン画面でXorg/X11を選んでください。Waylandでの記録はまだ対応していません。"
    );
    Ok(())
}
pub fn monitors() -> Result<Vec<Monitor>> {
    supported_session()?;
    let mut output = vec![];
    for monitor in
        xcap::Monitor::all().context("画面を取得できません。画面収録の権限を確認してください。")?
    {
        output.push(Monitor {
            id: monitor.id()?.to_string(),
            name: monitor.name()?,
            bounds: Bounds {
                x: monitor.x()?,
                y: monitor.y()?,
                width: monitor.width()?,
                height: monitor.height()?,
            },
        });
    }
    ensure!(!output.is_empty(), "画面が見つかりません。");
    Ok(output)
}
pub fn windows() -> Result<Vec<Window>> {
    supported_session()?;
    let mut output = vec![];
    for window in
        xcap::Window::all().context("ウィンドウを取得できません。黒塗りのため記録を停止します。")?
    {
        if window.is_minimized()? {
            continue;
        }
        output.push(Window {
            process: window.app_name()?,
            title: window.title()?,
            bounds: Bounds {
                x: window.x()?,
                y: window.y()?,
                width: window.width()?,
                height: window.height()?,
            },
        });
    }
    Ok(output)
}
pub fn capture(monitor: &Monitor) -> Result<RgbImage> {
    supported_session()?;
    #[cfg(target_os = "macos")]
    ensure!(
        mac::capture_allowed(),
        "システム設定 → プライバシーとセキュリティ → 画面収録でTickreelを許可してください。"
    );
    let source = xcap::Monitor::all()?
        .into_iter()
        .find(|m| m.id().is_ok_and(|id| id.to_string() == monitor.id))
        .context("画面の接続が変更されました。")?;
    let captured = source.capture_image()?;
    let rgb = image::DynamicImage::ImageRgba8(captured).to_rgb8();
    // macOS windows and monitor origins are in points. Normalize Retina captures
    // to the same coordinate space before applying any privacy masks.
    Ok(
        if rgb.dimensions() == (monitor.bounds.width, monitor.bounds.height) {
            rgb
        } else {
            imageops::resize(
                &rgb,
                monitor.bounds.width,
                monitor.bounds.height,
                FilterType::Triangle,
            )
        },
    )
}
#[cfg(target_os = "linux")]
fn screen_state() -> Result<(u32, bool)> {
    use x11rb::{
        connection::Connection,
        protocol::screensaver::{ConnectionExt, State},
    };
    let (connection, screen) = x11rb::connect(None)?;
    let info = connection
        .screensaver_query_info(connection.setup().roots[screen].root)?
        .reply()?;
    Ok((
        info.ms_since_user_input / 1000,
        info.state != u8::from(State::ON),
    ))
}
pub fn idle_seconds() -> u32 {
    #[cfg(target_os = "linux")]
    {
        screen_state().map(|v| v.0).unwrap_or(0)
    }
    #[cfg(target_os = "macos")]
    {
        mac::idle_seconds()
    }
}
pub fn desktop_unlocked() -> bool {
    #[cfg(target_os = "linux")]
    {
        screen_state().map(|v| v.1).unwrap_or(false)
    }
    #[cfg(target_os = "macos")]
    {
        mac::unlocked()
    }
}
// The engine also detects long capture gaps after sleep and pauses the session.
pub fn keep_awake(_active: bool) {}
pub fn open(path: &std::path::Path, reveal: bool) {
    #[cfg(target_os = "macos")]
    {
        let mut command = std::process::Command::new("open");
        if reveal {
            command.arg("-R");
        }
        let _ = command.arg(path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let selected = if reveal {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        let _ = std::process::Command::new("xdg-open").arg(selected).spawn();
    }
}
include!("native_image.rs");

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{c_char, c_void};
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
        fn CGSessionCopyCurrentDictionary() -> *const c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(
            allocator: *const c_void,
            text: *const c_char,
            encoding: u32,
        ) -> *const c_void;
        fn CFDictionaryGetValue(dictionary: *const c_void, key: *const c_void) -> *const c_void;
        fn CFBooleanGetValue(value: *const c_void) -> bool;
        fn CFRelease(value: *const c_void);
    }
    pub fn capture_allowed() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() || CGRequestScreenCaptureAccess() }
    }
    pub fn idle_seconds() -> u32 {
        unsafe { CGEventSourceSecondsSinceLastEventType(1, u32::MAX).max(0.0) as u32 }
    }
    pub fn unlocked() -> bool {
        unsafe {
            let dictionary = CGSessionCopyCurrentDictionary();
            if dictionary.is_null() {
                return false;
            }
            let key = CFStringCreateWithCString(
                std::ptr::null(),
                c"CGSSessionScreenIsLocked".as_ptr(),
                0x08000100,
            );
            if key.is_null() {
                CFRelease(dictionary);
                return false;
            }
            let value = CFDictionaryGetValue(dictionary, key);
            let unlocked = value.is_null() || !CFBooleanGetValue(value);
            CFRelease(key);
            CFRelease(dictionary);
            unlocked
        }
    }
}
