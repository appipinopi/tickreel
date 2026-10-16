use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub portal_selection: bool,
    pub app_redaction: bool,
    pub idle_detection: bool,
}

fn wayland_session(session_type: &str, display: &str) -> bool {
    session_type.eq_ignore_ascii_case("wayland") || !display.is_empty()
}

pub fn is_wayland() -> bool {
    cfg!(target_os = "linux")
        && wayland_session(
            &std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
            &std::env::var("WAYLAND_DISPLAY").unwrap_or_default(),
        )
}

pub fn capabilities() -> Capabilities {
    let wayland = is_wayland();
    Capabilities {
        portal_selection: wayland,
        app_redaction: !wayland,
        idle_detection: !wayland,
    }
}

pub fn shutdown() {
    #[cfg(target_os = "linux")]
    crate::wayland::release();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_wayland_even_with_xwayland_display() {
        assert!(wayland_session("wayland", ""));
        assert!(wayland_session("", "custom-socket"));
        assert!(wayland_session("Wayland", "wayland-0"));
        assert!(!wayland_session("x11", ""));
        assert!(!wayland_session("", ""));
    }
}
