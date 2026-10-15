#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod engine;
mod export;
mod model;
#[cfg(windows)]
mod native;
#[cfg(not(windows))]
#[path = "native_unix.rs"]
mod native;
mod store;
mod ui;
#[cfg(windows)]
mod updater;
#[cfg(not(windows))]
#[path = "updater_unix.rs"]
mod updater;
mod web_ui;

fn main() {
    native::set_dpi();
    if std::env::args().any(|arg| arg == "--check-update") {
        match updater::Updater::new().check() {
            Ok(status) => println!("{}", serde_json::to_string(&status).unwrap()),
            Err(error) => {
                eprintln!("{error:#}");
                std::process::exit(1);
            }
        }
        return;
    }
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments.get(1).is_some_and(|v| v == "--apply-update") {
        let result = (|| -> anyhow::Result<()> {
            anyhow::ensure!(arguments.len() == 5, "更新の引数が不正です。");
            updater::apply(
                arguments[2].clone().into(),
                arguments[3].clone().into(),
                arguments[4].to_string_lossy().parse()?,
            )
        })();
        if let Err(error) = result {
            rfd::MessageDialog::new()
                .set_title("Tickreel 更新エラー")
                .set_description(format!("更新を適用できませんでした。\n{error:#}"))
                .set_level(rfd::MessageLevel::Error)
                .show();
            std::process::exit(1);
        }
        return;
    }
    if std::env::args().any(|argument| argument == "--self-test") {
        if let Err(error) = engine::self_test() {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        println!(
            "PASS: Rust native capture, privacy, storage, idle compression, audio, export and summaries"
        );
        return;
    }
    let result = if std::env::args().any(|argument| argument == "--egui") {
        ui::run()
    } else {
        web_ui::run()
    };
    if let Err(error) = result {
        rfd::MessageDialog::new()
            .set_title("Tickreel Rust")
            .set_description(format!("起動できませんでした。\n{error:#}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
