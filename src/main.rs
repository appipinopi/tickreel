#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod engine;
mod export;
mod model;
mod native;
mod store;
mod ui;
mod web_ui;

fn main() {
    native::set_dpi();
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
