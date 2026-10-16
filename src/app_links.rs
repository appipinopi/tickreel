use app_signature::{LiveSignature, Signature};
use std::sync::{Mutex, OnceLock};

fn state() -> &'static Mutex<Option<LiveSignature>> {
    static STATE: OnceLock<Mutex<Option<LiveSignature>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}
pub fn start() {
    let mut state = state().lock().unwrap();
    if state.is_none() {
        let cache = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("TickreelRust/app-signature/creator.json");
        *state = Some(LiveSignature::new(embedded().clone(), cache));
    }
}
#[allow(dead_code)]
pub fn shutdown() {
    let previous = state().lock().unwrap().take();
    drop(previous);
}
pub fn signature() -> Signature {
    state()
        .lock()
        .unwrap()
        .as_ref()
        .map(LiveSignature::snapshot)
        .unwrap_or_else(|| embedded().clone())
}
pub fn open_works() -> anyhow::Result<()> {
    let current = std::env::current_exe()?;
    let host = current
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let executable = if host == "obs64" || host == "obs32" {
        current
            .parent()
            .and_then(|path| path.parent())
            .and_then(|path| path.parent())
            .ok_or_else(|| anyhow::anyhow!("OBSフォルダーが見つかりません。"))?
            .join("obs-plugins/64bit/tickreel-gallery.exe")
    } else {
        current
    };
    anyhow::ensure!(
        executable.is_file(),
        "アプリ一覧用の実行ファイルがありません。プラグインのZIPを再導入してください。"
    );
    std::process::Command::new(executable)
        .arg("--creator-apps")
        .spawn()?;
    Ok(())
}

fn embedded() -> &'static Signature {
    static SIGNATURE: OnceLock<Signature> = OnceLock::new();
    SIGNATURE.get_or_init(|| {
        Signature::from_json(include_str!("../assets/app-signature.json"))
            .expect("embedded app signature is valid")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_links_are_configured_and_arbitrary_targets_are_rejected() {
        assert_eq!(
            signature().resolve("website").unwrap(),
            "https://tickreel.ycccompany.jp/"
        );
        assert_eq!(
            signature().resolve("creator").unwrap(),
            "https://github.com/appipinopi"
        );
        assert!(
            signature()
                .resolve("file:///C:/Windows/system32/cmd.exe")
                .is_err()
        );
    }
}
