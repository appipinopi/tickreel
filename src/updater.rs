use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

const ARCH: &str = if cfg!(target_arch = "aarch64") {
    "arm64"
} else if cfg!(target_arch = "x86") {
    "x86"
} else {
    "x64"
};
fn endpoint() -> String {
    format!(
        "https://github.com/appipinopi/tickreel/releases/latest/download/update-windows-{ARCH}.json"
    )
}
const KEY: &[u8; 32] = include_bytes!("../assets/update-public-key.bin");
const LIMIT: u64 = 100 * 1024 * 1024;

#[derive(Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub version: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub signature: String,
}
impl Manifest {
    fn signed_bytes(&self) -> String {
        format!(
            "Tickreel update\n{}\n{}\n{}\n{}\n",
            self.version, self.url, self.size, self.sha256
        )
    }
    fn verify(&self, key: &[u8; 32]) -> Result<()> {
        semver::Version::parse(&self.version)?;
        ensure!(
            self.size > 0 && self.size <= LIMIT,
            "更新ファイルのサイズが不正です。"
        );
        ensure!(
            self.url
                .starts_with("https://github.com/appipinopi/tickreel/releases/download/")
                && self.url.ends_with(&format!(
                    "/Tickreel-{}-windows-{ARCH}-portable.exe",
                    self.version
                )),
            "更新の配布先が不正です。"
        );
        ensure!(
            self.sha256.len() == 64 && self.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "更新のハッシュが不正です。"
        );
        let signature = Signature::from_slice(&STANDARD.decode(&self.signature)?)?;
        VerifyingKey::from_bytes(key)?
            .verify_strict(self.signed_bytes().as_bytes(), &signature)
            .context("更新ファイルの署名を確認できません。")
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    current: &'static str,
    channel: &'static str,
    pub phase: String,
    pub version: Option<String>,
    pub message: String,
}
struct Inner {
    status: Status,
    manifest: Option<Manifest>,
    staged: Option<tempfile::TempDir>,
}
#[derive(Clone)]
pub struct Updater(Arc<Mutex<Inner>>);

pub fn store_managed() -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentPackageFullName(length: *mut u32, name: *mut u16) -> i32;
    }
    let mut length = 0;
    let packaged = unsafe { GetCurrentPackageFullName(&mut length, std::ptr::null_mut()) } != 15700;
    packaged || option_env!("TICKREEL_CHANNEL") == Some("store")
}
fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .https_only(true)
        .user_agent(concat!("Tickreel/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()?)
}
fn digest(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
impl Updater {
    pub fn new() -> Self {
        let store = store_managed();
        Self(Arc::new(Mutex::new(Inner {
            status: Status {
                current: env!("CARGO_PKG_VERSION"),
                channel: if store { "store" } else { "github" },
                phase: if store { "store" } else { "idle" }.into(),
                version: None,
                message: if store {
                    "Microsoft Storeが更新を管理します。"
                } else {
                    "起動時に更新を確認します。"
                }
                .into(),
            },
            manifest: None,
            staged: None,
        })))
    }
    pub fn status(&self) -> Status {
        self.0.lock().unwrap().status.clone()
    }
    pub fn check(&self) -> Result<Status> {
        {
            let mut inner = self.0.lock().unwrap();
            if inner.status.channel == "store"
                || ["checking", "downloading", "ready"].contains(&inner.status.phase.as_str())
            {
                return Ok(inner.status.clone());
            }
            inner.status.phase = "checking".into();
            inner.status.message = "更新を確認しています…".into();
        }
        let result = (|| -> Result<Option<Manifest>> {
            let response = client()?
                .get(endpoint())
                .timeout(Duration::from_secs(20))
                .send()?;
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            let mut bytes = Vec::new();
            response
                .error_for_status()?
                .take(65537)
                .read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= 65536, "更新情報が大きすぎます。");
            let manifest: Manifest = serde_json::from_slice(&bytes)?;
            manifest.verify(KEY)?;
            let next = semver::Version::parse(&manifest.version)?;
            let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
            Ok(if next > current && next.pre.is_empty() {
                Some(manifest)
            } else {
                None
            })
        })();
        let mut inner = self.0.lock().unwrap();
        match result {
            Ok(manifest) => {
                inner.status.version = manifest.as_ref().map(|m| m.version.clone());
                inner.status.phase = if manifest.is_some() {
                    "available"
                } else {
                    "current"
                }
                .into();
                inner.status.message = if manifest.is_some() {
                    "新しいバージョンがあります。"
                } else {
                    "最新バージョンです。"
                }
                .into();
                inner.manifest = manifest;
            }
            Err(error) => {
                inner.status.phase = "error".into();
                inner.status.message =
                    format!("更新を確認できませんでした。再度お試しください。 {error}");
            }
        }
        Ok(inner.status.clone())
    }
    pub fn download(&self) -> Result<Status> {
        let manifest = {
            let mut inner = self.0.lock().unwrap();
            ensure!(
                inner.status.channel == "github" && inner.status.phase == "available",
                "ダウンロードできる更新がありません。"
            );
            let manifest = inner.manifest.clone().context("更新を確認してください。")?;
            inner.status.phase = "downloading".into();
            inner.status.message = "更新をダウンロードしています…".into();
            manifest
        };
        let result = (|| -> Result<tempfile::TempDir> {
            manifest.verify(KEY)?;
            let root = dirs::data_local_dir()
                .context("更新フォルダーがありません。")?
                .join("TickreelUpdates");
            fs::create_dir_all(&root)?;
            let stage = tempfile::Builder::new()
                .prefix("update-")
                .tempdir_in(root)?;
            let path = stage.path().join("Tickreel.exe");
            let mut file = fs::File::create(&path)?;
            let response = client()?.get(&manifest.url).send()?.error_for_status()?;
            let count = std::io::copy(&mut response.take(manifest.size + 1), &mut file)?;
            file.flush()?;
            file.sync_all()?;
            drop(file);
            ensure!(
                count == manifest.size && digest(&path)? == manifest.sha256,
                "更新ファイルが破損しています。"
            );
            // Persist the signed description for an independent verification immediately before replacement.
            fs::write(
                stage.path().join("manifest.json"),
                serde_json::to_vec(&manifest)?,
            )?;
            Ok(stage)
        })();
        let mut inner = self.0.lock().unwrap();
        match result {
            Ok(stage) => {
                inner.staged = Some(stage);
                inner.status.phase = "ready".into();
                inner.status.message =
                    "更新の準備ができました。アプリ終了時に適用し、再起動します。".into();
            }
            Err(error) => {
                inner.status.phase = "available".into();
                inner.status.message = format!("更新の取得に失敗しました。 {error}");
                return Err(error);
            }
        }
        Ok(inner.status.clone())
    }
    pub fn launch_on_exit(&self) -> Result<()> {
        let mut inner = self.0.lock().unwrap();
        let Some(stage) = inner.staged.as_ref() else {
            return Ok(());
        };
        ensure!(
            !store_managed(),
            "Store版はMicrosoft Storeから更新してください。"
        );
        let target = std::env::current_exe()?.canonicalize()?;
        let manifest: Manifest =
            serde_json::from_slice(&fs::read(stage.path().join("manifest.json"))?)?;
        manifest.verify(KEY)?;
        ensure!(
            digest(&stage.path().join("Tickreel.exe"))? == manifest.sha256,
            "更新ファイルが変更されています。"
        );
        let helper = stage.path().join("Tickreel-UpdateHelper.exe");
        fs::copy(&target, &helper)?;
        crate::export::hidden_command(&helper)
            .arg("--apply-update")
            .arg(stage.path())
            .arg(&target)
            .arg(std::process::id().to_string())
            .spawn()?;
        // The helper now owns the temporary directory. Do not delete its executable on drop.
        let _ = inner.staged.take().unwrap().keep();
        Ok(())
    }
}

pub fn apply(stage: PathBuf, target: PathBuf, parent: u32) -> Result<()> {
    ensure!(!store_managed(), "Store版の差し替えはできません。");
    let stage = stage.canonicalize()?;
    let helper = std::env::current_exe()?.canonicalize()?;
    ensure!(
        helper.parent() == Some(stage.as_path())
            && helper
                .file_name()
                .is_some_and(|n| n == "Tickreel-UpdateHelper.exe"),
        "更新ヘルパーの場所が不正です。"
    );
    ensure!(
        target.is_absolute()
            && target
                .extension()
                .is_some_and(|v| v.eq_ignore_ascii_case("exe")),
        "更新対象が不正です。"
    );
    let manifest: Manifest = serde_json::from_slice(&fs::read(stage.join("manifest.json"))?)?;
    manifest.verify(KEY)?;
    let source = stage.join("Tickreel.exe");
    ensure!(
        fs::metadata(&source)?.len() == manifest.size && digest(&source)? == manifest.sha256,
        "更新ファイルを検証できません。"
    );
    use windows_sys::Win32::{
        Foundation::{CloseHandle, GetLastError, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, parent);
        if !handle.is_null() {
            let result = WaitForSingleObject(handle, 300000);
            CloseHandle(handle);
            ensure!(
                result == WAIT_OBJECT_0,
                "アプリの終了を待ちきれませんでした。"
            );
        } else {
            ensure!(GetLastError() == 87, "アプリの終了を確認できません。");
        }
    }
    replace(&source, &target)?;
    crate::export::hidden_command(&target).spawn()?;
    // Only known staging files are removed; the running helper itself is left for Windows to release.
    let _ = fs::remove_file(source);
    let _ = fs::remove_file(stage.join("manifest.json"));
    Ok(())
}
fn replace(source: &Path, target: &Path) -> Result<()> {
    let parent = target.parent().context("更新先がありません。")?;
    let staged = tempfile::Builder::new()
        .prefix(".tickreel-new-")
        .suffix(".exe")
        .tempfile_in(parent)?
        .into_temp_path();
    fs::copy(source, &staged)?;
    let backup = parent.join(format!(".tickreel-backup-{}.exe", uuid::Uuid::new_v4()));
    fs::rename(target, &backup)
        .context("アプリを書き換えられません。保存場所の権限を確認してください。")?;
    if let Err(error) = staged.persist(target) {
        fs::rename(&backup, target).context("旧版の復元に失敗しました。")?;
        return Err(error.into());
    }
    let _ = fs::remove_file(backup);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    #[test]
    fn signed_manifest_rejects_tampering() {
        let key = SigningKey::from_bytes(&[17; 32]);
        let mut m = Manifest {
            version: "1.0.1".into(),
            url: format!(
                "https://github.com/appipinopi/tickreel/releases/download/v1.0.1/Tickreel-1.0.1-windows-{ARCH}-portable.exe"
            ),
            size: 4,
            sha256: "00".repeat(32),
            signature: String::new(),
        };
        m.signature = STANDARD.encode(key.sign(m.signed_bytes().as_bytes()).to_bytes());
        assert!(m.verify(key.verifying_key().as_bytes()).is_ok());
        let correct_url = m.url.clone();
        let wrong_arch = if ARCH == "arm64" { "x64" } else { "arm64" };
        m.url = m.url.replace(
            &format!("windows-{ARCH}-"),
            &format!("windows-{wrong_arch}-"),
        );
        m.signature = STANDARD.encode(key.sign(m.signed_bytes().as_bytes()).to_bytes());
        assert!(m.verify(key.verifying_key().as_bytes()).is_err());
        m.url = correct_url;
        m.signature = STANDARD.encode(key.sign(m.signed_bytes().as_bytes()).to_bytes());
        m.size = 5;
        assert!(m.verify(key.verifying_key().as_bytes()).is_err());
    }
    #[test]
    fn replacement_keeps_unrelated_files() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("app.exe");
        let source = dir.path().join("new.exe");
        fs::write(&target, b"old").unwrap();
        fs::write(&source, b"new").unwrap();
        fs::write(dir.path().join("recordings.json"), b"recordings").unwrap();
        replace(&source, &target).unwrap();
        assert_eq!(fs::read(target).unwrap(), b"new");
        assert_eq!(
            fs::read(dir.path().join("recordings.json")).unwrap(),
            b"recordings"
        );
    }
}
