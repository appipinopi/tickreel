use anyhow::{Context, Result};
use image::{
    Rgb, RgbImage,
    imageops::{self, FilterType},
};
use std::{
    mem::{size_of, zeroed},
    ptr,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{Power::*, StationsAndDesktops::*, SystemInformation::*, Threading::*},
    UI::{HiDpi::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
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
pub fn set_dpi() {
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
unsafe extern "system" fn monitor_callback(
    handle: HMONITOR,
    _: HDC,
    _: *mut RECT,
    data: LPARAM,
) -> i32 {
    let mut info: MONITORINFOEXW = unsafe { zeroed() };
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    if unsafe { GetMonitorInfoW(handle, &mut info as *mut _ as *mut MONITORINFO) } != 0 {
        let id = String::from_utf16_lossy(
            &info.szDevice[..info
                .szDevice
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(info.szDevice.len())],
        );
        let rect = info.monitorInfo.rcMonitor;
        unsafe { &mut *(data as *mut Vec<Monitor>) }.push(Monitor {
            name: format!(
                "{} · {}×{}",
                id,
                rect.right - rect.left,
                rect.bottom - rect.top
            ),
            id,
            bounds: Bounds {
                x: rect.left,
                y: rect.top,
                width: (rect.right - rect.left) as u32,
                height: (rect.bottom - rect.top) as u32,
            },
        });
    }
    1
}
pub fn monitors() -> Result<Vec<Monitor>> {
    set_dpi();
    let mut result = vec![];
    anyhow::ensure!(
        unsafe {
            EnumDisplayMonitors(
                ptr::null_mut(),
                ptr::null(),
                Some(monitor_callback),
                &mut result as *mut _ as LPARAM,
            )
        } != 0,
        "モニターを検出できません。"
    );
    anyhow::ensure!(!result.is_empty(), "モニターが見つかりません。");
    Ok(result)
}
struct WindowEnumeration {
    items: Vec<Window>,
    failed: bool,
}
unsafe extern "system" fn window_callback(handle: HWND, data: LPARAM) -> i32 {
    let enumeration = unsafe { &mut *(data as *mut WindowEnumeration) };
    if unsafe { IsWindowVisible(handle) } == 0 || unsafe { IsIconic(handle) } != 0 {
        return 1;
    }
    let mut rect: RECT = unsafe { zeroed() };
    if unsafe { GetWindowRect(handle, &mut rect) } == 0 {
        enumeration.failed = true;
        return 1;
    }
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return 1;
    }
    let mut process_id = 0;
    unsafe {
        GetWindowThreadProcessId(handle, &mut process_id);
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process.is_null() {
        if unsafe { IsWindow(handle) } != 0 {
            enumeration.failed = true;
        }
        return 1;
    }
    let mut executable = [0u16; 4096];
    let mut length = executable.len() as u32;
    let success =
        unsafe { QueryFullProcessImageNameW(process, 0, executable.as_mut_ptr(), &mut length) };
    unsafe {
        CloseHandle(process);
    }
    if success == 0 {
        enumeration.failed = true;
        return 1;
    }
    let executable = String::from_utf16_lossy(&executable[..length as usize]);
    let process = std::path::Path::new(&executable)
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut title = [0u16; 1024];
    let count = unsafe { GetWindowTextW(handle, title.as_mut_ptr(), title.len() as i32) };
    enumeration.items.push(Window {
        process,
        title: String::from_utf16_lossy(&title[..count.max(0) as usize]),
        bounds: Bounds {
            x: rect.left,
            y: rect.top,
            width: (rect.right - rect.left) as u32,
            height: (rect.bottom - rect.top) as u32,
        },
    });
    1
}
pub fn windows() -> Result<Vec<Window>> {
    set_dpi();
    let mut enumeration = WindowEnumeration {
        items: vec![],
        failed: false,
    };
    anyhow::ensure!(
        unsafe { EnumWindows(Some(window_callback), &mut enumeration as *mut _ as LPARAM) } != 0
            && !enumeration.failed,
        "ウィンドウの位置を取得できません。黒塗りの安全のため記録を一時停止します。"
    );
    Ok(enumeration.items)
}
pub fn idle_seconds() -> u32 {
    unsafe {
        let mut info: LASTINPUTINFO = zeroed();
        info.cbSize = size_of::<LASTINPUTINFO>() as u32;
        if GetLastInputInfo(&mut info) == 0 {
            0
        } else {
            GetTickCount().wrapping_sub(info.dwTime) / 1000
        }
    }
}
pub fn desktop_unlocked() -> bool {
    unsafe {
        let desktop = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
        if desktop.is_null() {
            return false;
        }
        let mut name = [0u16; 128];
        let mut length = 0;
        let ok = GetUserObjectInformationW(
            desktop,
            UOI_NAME,
            name.as_mut_ptr().cast(),
            size_of_val(&name) as u32,
            &mut length,
        );
        CloseDesktop(desktop);
        ok != 0
            && String::from_utf16_lossy(&name[..name.iter().position(|v| *v == 0).unwrap_or(0)])
                .eq_ignore_ascii_case("Default")
    }
}
pub fn keep_awake(active: bool) {
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS | if active { ES_SYSTEM_REQUIRED } else { 0 });
    }
}
pub fn capture(monitor: &Monitor) -> Result<RgbImage> {
    set_dpi();
    let bounds = monitor.bounds;
    anyhow::ensure!(
        bounds.width > 0 && bounds.height > 0 && bounds.width <= 16384 && bounds.height <= 16384,
        "画面サイズを確認してください。"
    );
    unsafe {
        let screen = GetDC(ptr::null_mut());
        anyhow::ensure!(!screen.is_null(), "画面を取得できません。");
        let memory = CreateCompatibleDC(screen);
        if memory.is_null() {
            ReleaseDC(ptr::null_mut(), screen);
            anyhow::bail!("キャプチャ用DCを作成できません。");
        }
        let bitmap = CreateCompatibleBitmap(screen, bounds.width as i32, bounds.height as i32);
        if bitmap.is_null() {
            DeleteDC(memory);
            ReleaseDC(ptr::null_mut(), screen);
            anyhow::bail!("キャプチャ用画像を作成できません。");
        }
        let previous = SelectObject(memory, bitmap);
        let copied = BitBlt(
            memory,
            0,
            0,
            bounds.width as i32,
            bounds.height as i32,
            screen,
            bounds.x,
            bounds.y,
            SRCCOPY | CAPTUREBLT,
        );
        SelectObject(memory, previous);
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = bounds.width as i32;
        info.bmiHeader.biHeight = -(bounds.height as i32);
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut pixels = vec![0u8; bounds.width as usize * bounds.height as usize * 4];
        let rows = if copied != 0 {
            GetDIBits(
                memory,
                bitmap,
                0,
                bounds.height,
                pixels.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            )
        } else {
            0
        };
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(ptr::null_mut(), screen);
        anyhow::ensure!(
            rows == bounds.height as i32,
            "画面キャプチャに失敗しました。"
        );
        let mut rgb = Vec::with_capacity(bounds.width as usize * bounds.height as usize * 3);
        for pixel in pixels.chunks_exact(4) {
            rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
        RgbImage::from_raw(bounds.width, bounds.height, rgb).context("画像を読み込めません。")
    }
}
pub fn redact(image: &mut RgbImage, screen: Bounds, windows: &[Window], protected: &[String]) {
    for window in windows.iter().filter(|window| {
        protected
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&window.process))
    }) {
        let rect = window.bounds;
        let pad = 8;
        let x0 = (rect.x - screen.x - pad).max(0) as u32;
        let y0 = (rect.y - screen.y - pad).max(0) as u32;
        let x1 = (rect.x as i64 + rect.width as i64 - screen.x as i64 + pad as i64)
            .clamp(0, image.width() as i64) as u32;
        let y1 = (rect.y as i64 + rect.height as i64 - screen.y as i64 + pad as i64)
            .clamp(0, image.height() as i64) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                image.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }
    }
}
pub fn compose(images: &[RgbImage], horizontal: bool) -> Result<RgbImage> {
    anyhow::ensure!(
        !images.is_empty() && images.len() <= 8,
        "画面を1〜8枚選んでください。"
    );
    let columns = if horizontal {
        images.len() as u32
    } else {
        (images.len() as f64).sqrt().ceil() as u32
    };
    let rows = (images.len() as u32).div_ceil(columns);
    let cell_w = 1920 / columns;
    let cell_h = 1080 / rows;
    let mut output = RgbImage::from_pixel(1920, 1080, Rgb([17, 26, 29]));
    for (index, image) in images.iter().enumerate() {
        let scale =
            (cell_w as f64 / image.width() as f64).min(cell_h as f64 / image.height() as f64);
        let w = (image.width() as f64 * scale).round().max(1.0) as u32;
        let h = (image.height() as f64 * scale).round().max(1.0) as u32;
        let tile = imageops::resize(image, w.min(cell_w), h.min(cell_h), FilterType::Triangle);
        imageops::replace(
            &mut output,
            &tile,
            ((index as u32 % columns) * cell_w + (cell_w - tile.width()) / 2) as i64,
            ((index as u32 / columns) * cell_h + (cell_h - tile.height()) / 2) as i64,
        );
    }
    Ok(output)
}
pub fn open(path: &std::path::Path, reveal: bool) {
    unsafe {
        let verb = wide("open");
        let target = if reveal {
            wide("explorer.exe")
        } else {
            wide(&path.to_string_lossy())
        };
        let args = wide(&format!("/select,\"{}\"", path.display()));
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            if reveal { args.as_ptr() } else { ptr::null() },
            ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn privacy_handles_negative_screen_coordinates() {
        let mut image = RgbImage::from_pixel(100, 100, Rgb([255, 255, 255]));
        redact(
            &mut image,
            Bounds {
                x: -100,
                y: 0,
                width: 100,
                height: 100,
            },
            &[Window {
                process: "Secret".into(),
                title: String::new(),
                bounds: Bounds {
                    x: -60,
                    y: 20,
                    width: 40,
                    height: 40,
                },
            }],
            &["secret".into()],
        );
        assert_eq!(*image.get_pixel(50, 40), Rgb([0, 0, 0]));
        assert_eq!(*image.get_pixel(0, 0), Rgb([255, 255, 255]));
    }
}
