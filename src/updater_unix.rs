use anyhow::{Result, bail};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    current: &'static str,
    channel: &'static str,
    pub phase: String,
    pub version: Option<String>,
    pub message: String,
}
#[derive(Clone)]
pub struct Updater(Arc<Mutex<Status>>);
impl Updater {
    pub fn new() -> Self {
        let snap = std::env::var_os("SNAP").is_some();
        Self(Arc::new(Mutex::new(Status {
            current: env!("CARGO_PKG_VERSION"),
            channel: if snap { "snap" } else { "github" },
            phase: if snap { "store" } else { "manual" }.into(),
            version: None,
            message: if snap {
                "更新はSnap Storeが管理します。"
            } else {
                "macOS・Linux版はGitHub Releasesから最新版を取得してください。"
            }
            .into(),
        })))
    }
    pub fn status(&self) -> Status {
        self.0.lock().unwrap().clone()
    }
    pub fn check(&self) -> Result<Status> {
        Ok(self.status())
    }
    pub fn download(&self) -> Result<Status> {
        bail!("このOSでは配布ページから更新してください。")
    }
    pub fn launch_on_exit(&self) -> Result<()> {
        Ok(())
    }
}
pub fn apply(_stage: PathBuf, _target: PathBuf, _parent: u32) -> Result<()> {
    bail!("EXEの自動差し替えはWindows版専用です。")
}
