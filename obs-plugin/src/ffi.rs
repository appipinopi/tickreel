//! Minimal OBS 31 ABI. SourceInfo is deliberately a prefix: libobs's
//! obs_register_source_s copies `size` bytes and zero-fills newer fields.
//! Definitions: obsproject/obs-studio tag 31.0.0, libobs/obs-source.h,
//! obs-properties.h and media-io/video-io.h. Keep enum values/layouts in sync.
use libloading::Library;
use std::ffi::{c_char, c_void};

pub type Ptr = *mut c_void;
pub type Button = unsafe extern "C" fn(Ptr, Ptr, Ptr) -> bool;
pub type VideoCallback = unsafe extern "C" fn(Ptr, *mut VideoData);

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
