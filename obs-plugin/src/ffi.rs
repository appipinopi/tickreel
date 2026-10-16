//! Minimal OBS 31 ABI. SourceInfo is deliberately a prefix: libobs's
//! obs_register_source_s copies `size` bytes and zero-fills newer fields.
//! Definitions: obsproject/obs-studio tag 31.0.0, libobs/obs-source.h,
//! obs-properties.h and media-io/video-io.h. Keep enum values/layouts in sync.
use libloading::Library;
use std::ffi::{c_char, c_void};

pub type Ptr = *mut c_void;
pub type Button = unsafe extern "C" fn(Ptr, Ptr, Ptr) -> bool;
pub type VideoCallback = unsafe extern "C" fn(Ptr, *mut VideoData);
pub type FrontendCallback = unsafe extern "C" fn(u32, Ptr);

pub const RECORDING_STARTED: u32 = 5;
pub const RECORDING_STOPPED: u32 = 7;
pub const RECORDING_PAUSED: u32 = 27;
pub const RECORDING_UNPAUSED: u32 = 28;

pub struct FrontendApi {
    pub add_event: unsafe extern "C" fn(FrontendCallback, Ptr),
    pub remove_event: unsafe extern "C" fn(FrontendCallback, Ptr),
    pub recording_active: unsafe extern "C" fn() -> bool,
    _library: Library,
}
impl FrontendApi {
    pub unsafe fn load() -> anyhow::Result<Self> {
        #[cfg(windows)]
        let library = unsafe { Library::new("obs-frontend-api.dll")? };
        #[cfg(target_os = "linux")]
        let library = unsafe { Library::new("libobs-frontend-api.so.0")? };
        #[cfg(target_os = "macos")]
        let library = unsafe { Library::new("@rpath/libobs-frontend-api.dylib")? };
        let main_window: unsafe extern "C" fn() -> Ptr =
            unsafe { *library.get(b"obs_frontend_get_main_window\0")? };
        anyhow::ensure!(
            !unsafe { main_window() }.is_null(),
            "OBS frontend is not initialized"
        );
        Ok(Self {
            add_event: unsafe { *library.get(b"obs_frontend_add_event_callback\0")? },
            remove_event: unsafe { *library.get(b"obs_frontend_remove_event_callback\0")? },
            recording_active: unsafe { *library.get(b"obs_frontend_recording_active\0")? },
            _library: library,
        })
    }
}

#[repr(C)]
pub struct SourceInfo {
    pub id: *const c_char,
    pub kind: u32,
    pub flags: u32,
    pub name: unsafe extern "C" fn(Ptr) -> *const c_char,
    pub create: unsafe extern "C" fn(Ptr, Ptr) -> Ptr,
    pub destroy: unsafe extern "C" fn(Ptr),
    pub width: Option<unsafe extern "C" fn(Ptr) -> u32>,
    pub height: Option<unsafe extern "C" fn(Ptr) -> u32>,
    pub defaults: unsafe extern "C" fn(Ptr),
    pub properties: unsafe extern "C" fn(Ptr) -> Ptr,
    pub update: unsafe extern "C" fn(Ptr, Ptr),
}

#[repr(C)]
pub struct VideoData {
    pub data: [*mut u8; 8],
    pub linesize: [u32; 8],
    pub timestamp: u64,
}

#[repr(C)]
pub struct ScaleInfo {
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub range: u32,
    pub colorspace: u32,
}

macro_rules! obs_api {
    ($($name:ident: $ty:ty),* $(,)?) => {
        pub struct Api {
            $(pub $name: $ty,)*
            _library: Library,
        }
        impl Api {
            pub unsafe fn load() -> anyhow::Result<Self> {
                #[cfg(windows)]
                let library = unsafe { Library::new("obs.dll")? };
                #[cfg(target_os = "linux")]
                let library = unsafe { Library::new("libobs.so.0")? };
                #[cfg(target_os = "macos")]
                let library = unsafe { Library::new("@rpath/libobs.framework/Versions/A/libobs")? };
                Ok(Self {
                    $($name: unsafe { *library.get(concat!(stringify!($name), "\0").as_bytes())? },)*
                    _library: library,
                })
            }
        }
    }
}

obs_api! {
    obs_register_source_s: unsafe extern "C" fn(*const SourceInfo, usize),
    obs_data_get_int: unsafe extern "C" fn(Ptr, *const c_char) -> i64,
    obs_data_get_string: unsafe extern "C" fn(Ptr, *const c_char) -> *const c_char,
    obs_data_set_default_int: unsafe extern "C" fn(Ptr, *const c_char, i64),
    obs_data_set_default_string: unsafe extern "C" fn(Ptr, *const c_char, *const c_char),
    obs_properties_create: unsafe extern "C" fn() -> Ptr,
    obs_properties_add_list: unsafe extern "C" fn(Ptr, *const c_char, *const c_char, u32, u32) -> Ptr,
    obs_property_list_add_int: unsafe extern "C" fn(Ptr, *const c_char, i64) -> usize,
    obs_properties_add_text: unsafe extern "C" fn(Ptr, *const c_char, *const c_char, u32) -> Ptr,
    obs_properties_add_button: unsafe extern "C" fn(Ptr, *const c_char, *const c_char, Button) -> Ptr,
    obs_properties_get: unsafe extern "C" fn(Ptr, *const c_char) -> Ptr,
    obs_property_set_description: unsafe extern "C" fn(Ptr, *const c_char),
    obs_property_set_enabled: unsafe extern "C" fn(Ptr, bool),
    obs_get_video: unsafe extern "C" fn() -> Ptr,
    video_output_get_width: unsafe extern "C" fn(Ptr) -> u32,
    video_output_get_height: unsafe extern "C" fn(Ptr) -> u32,
    obs_add_raw_video_callback: unsafe extern "C" fn(*const ScaleInfo, VideoCallback, Ptr),
    obs_remove_raw_video_callback: unsafe extern "C" fn(VideoCallback, Ptr),
}
