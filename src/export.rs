use crate::{
    audio,
    model::{Edit, Recording, selected_frames},
    store::Store,
};
use anyhow::{Context, Result};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    collections::VecDeque,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
pub const END_CARD: &[u8] = include_bytes!("../assets/endcard.png");
pub fn ffmpeg() -> PathBuf {
    if let Some(path) = std::env::var_os("TICKREEL_FFMPEG") {
        return path.into();
    }
    if let Ok(executable) = std::env::current_exe() {
        let adjacent = executable.with_file_name(if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        });
        if adjacent.is_file() {
            return adjacent;
        }
    }
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(if cfg!(windows) {
        "node_modules/ffmpeg-static/ffmpeg.exe"
    } else {
        "node_modules/ffmpeg-static/ffmpeg"
    });
    if development.is_file() {
        development
    } else {
        if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        }
        .into()
    }
}
pub fn hidden_command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    command.stdin(Stdio::null());
    command
}
fn concat_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "'\\''")
}
fn filter_path(path: &Path) -> String {
    concat_path(path).replace(':', "\\:")
}
fn ass_time(seconds: f64) -> String {
    let cs = (seconds * 100.0).round() as u64;
    format!(
        "{}:{:02}:{:02}.{:02}",
        cs / 360000,
        cs / 6000 % 60,
        cs / 100 % 60,
        cs % 100
    )
}
fn ass_text(text: &str) -> String {
    text.replace('\\', "＼")
        .replace(['{', '}'], "")
        .replace('\r', "")
        .replace('\n', "\\N")
}
fn ass_color(color: &str) -> String {
    let hex = color.strip_prefix('#').unwrap_or("");
    if hex.len() == 6 && hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        format!("{}{}{}", &hex[4..6], &hex[2..4], &hex[0..2])
    } else {
        "FFFFFF".into()
    }
}
pub fn output_format(path: &Path) -> Result<String> {
    let format = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    anyhow::ensure!(
        [
            "mp4", "mov", "webm", "mkv", "avi", "gif", "wav", "mp3", "flac", "m4a", "ogg", "aac"
        ]
        .contains(&format.as_str()),
        "対応していない出力形式です。"
    );
    Ok(format)
}
fn subtitles(items: &[(Recording, f64, f64)], edit: &Edit) -> String {
    let font = if cfg!(windows) {
        "Yu Gothic"
    } else if cfg!(target_os = "macos") {
        "Hiragino Sans"
    } else {
        "Noto Sans CJK JP"
    };
    let color = if edit.theme == "terminal" {
        "&H00B2EDBE"
    } else {
        "&H00FFFFFF"
    };
    let mut text = format!(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: 1920\nPlayResY: 1080\nWrapStyle: 0\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Caption,{font},46,{color},&H00FFFFFF,&H001B2828,&H90000000,-1,0,0,0,100,100,0,0,1,3,1,2,65,65,88,1\nStyle: Stats,{font},24,&H00FFFFFF,&H00FFFFFF,&H001B2828,&H90000000,0,0,0,0,100,100,0,0,3,8,0,7,45,45,32,1\nStyle: Title,{font},42,{color},&H00FFFFFF,&H001B2828,&H90000000,-1,0,0,0,100,100,0,0,1,2,1,8,65,65,78,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n"
    );
    let mut event = |start: f64, end: f64, style: &str, value: String| {
        if end > start {
            text.push_str(&format!(
                "Dialogue: 0,{},{},{style},,0,0,0,,{value}\n",
                ass_time(start),
                ass_time(end)
            ));
        }
    };
    for (record, start, duration) in items {
        if edit.theme != "clean" {
            event(
                *start,
                (start + 2.5).min(start + duration),
                "Title",
                format!("{{\\fad(200,300)}}{}", ass_text(&record.title)),
            );
        }
        if edit.stats {
            event(
                *start,
                start + duration,
                "Stats",
                format!(
                    "{}   作業 {}分   無操作 {}分   {}フレーム   {}画面",
                    record.local_day(),
                    record.active_ms.saturating_sub(record.idle_ms) / 60000,
                    record.idle_ms / 60000,
                    record.frames,
                    record.source_ids.len().max(1)
                ),
            );
        }
        let captions = if items.len() == 1 {
            &edit.captions
        } else {
            &record.edit.captions
        };
        for caption in captions {
            event(
                start + caption.start,
                (start + caption.start + caption.duration).min(start + duration),
                "Caption",
                format!(
                    "{{\\an5\\pos({},{})\\fs{}\\c&H{}&\\fad(120,120)}}{}",
                    (caption.x.clamp(0.02, 0.98) * 1920.0).round(),
                    (caption.y.clamp(0.02, 0.98) * 1080.0).round(),
                    caption.size.clamp(16.0, 120.0),
                    ass_color(&caption.color),
                    ass_text(&caption.text)
                ),
            );
        }
    }
    text
}
#[derive(Clone, Debug)]
pub struct ExportResult {
    pub path: PathBuf,
    pub duration: f64,
    pub body_duration: f64,
}
pub fn generate(
    store: &Store,
    records: &[Recording],
    destination: &Path,
    edit: &Edit,
    mut progress: impl FnMut(u32),
) -> Result<ExportResult> {
    anyhow::ensure!(
        !records.is_empty()
            && records
                .iter()
                .all(|record| record.frames > 0 && [15, 24, 30, 60].contains(&record.fps)),
        "保存フレームがありません。fpsを確認してください。"
    );
    let format = output_format(destination)?;
    let audio_only = ["wav", "mp3", "flac", "m4a", "ogg", "aac"].contains(&format.as_str());
    let gif = format == "gif";
    let work = tempfile::tempdir()?;
    let fps = records[0].fps;
    let parent = destination
        .parent()
        .context("動画の保存先を確認してください。")?;
    fs::create_dir_all(parent)?;
    let output = tempfile::Builder::new()
        .prefix(".tickreel-")
        .suffix(&format!(".{format}"))
        .tempfile_in(parent)?
        .into_temp_path();
    let mut concat = String::from("ffconcat version 1.0\n");
    let mut body_duration = 0.0;
    let mut items = vec![];
    for record in records {
        let frames = selected_frames(record, &store.timeline(&record.id)?);
        let directory = store.directory(&record.id)?;
        for index in &frames {
            let file = directory.join(format!("frame-{index:07}.jpg"));
            anyhow::ensure!(
                file.is_file(),
                "記録のフレームが見つかりません: {}",
                file.display()
            );
            concat.push_str(&format!(
                "file '{}'\noption framerate {fps}\nduration {}\n",
                concat_path(&file),
                1.0 / record.fps as f64
            ));
        }
        let duration = frames.len() as f64 / record.fps as f64;
        items.push((record.clone(), body_duration, duration));
        body_duration += duration;
    }
    let card = work.path().join("end.jpg");
    image::load_from_memory(END_CARD)?.to_rgb8().save(&card)?;
    concat.push_str(&format!(
        "file '{}'\noption framerate {fps}\nduration 3\nfile '{}'\noption framerate {fps}\n",
        concat_path(&card),
        concat_path(&card)
    ));
    let duration = body_duration + if audio_only { 0.0 } else { 3.0 };
    let list = work.path().join("frames.txt");
    let titles = work.path().join("titles.ass");
    fs::write(&list, concat)?;
    fs::write(&titles, subtitles(&items, edit))?;
    let mut args = vec![
        "-y".into(),
        "-f".into(),
        "concat".into(),
        "-safe".into(),
        "0".into(),
        "-i".into(),
        list.to_string_lossy().into_owned(),
    ];
    let mut audio_filters = vec![];
    let mut labels = vec![];
    let mut audio_index = 1;
    if edit.bgm != "none" {
        let music = if edit.bgm == "custom" {
            let file = records[0]
                .bgm_path
                .as_ref()
                .context("BGMファイルを選んでください。")?;
            let basename = Path::new(file)
                .file_name()
                .context("BGMの保存先を確認してください。")?;
            store.directory(&records[0].id)?.join(basename)
        } else {
            let file = work.path().join("music.wav");
            audio::make_music(
                &file,
                edit.bgm == "focus" || edit.bgm == "auto" && records[0].category == "development",
            )?;
            file
        };
        args.extend([
            "-stream_loop".into(),
            "-1".into(),
            "-i".into(),
            music.to_string_lossy().into_owned(),
        ]);
        audio_filters.push(format!("[{audio_index}:a]volume={},atrim=duration={duration},afade=t=out:st={}:d=1.5,asetpts=PTS-STARTPTS[bgm]", edit.bgm_volume.clamp(0.0,1.0), (duration-1.5).max(0.0)));
        labels.push("bgm".to_owned());
        audio_index += 1;
    }
    for (index, (record, start, length)) in items.iter().enumerate() {
        if let Some(file) = &record.narration_path {
            let basename = Path::new(file)
                .file_name()
                .context("録音の保存先を確認してください。")?;
            let file = store.directory(&record.id)?.join(basename);
            let voice = if items.len() == 1 { edit } else { &record.edit };
            if voice.narration_offset >= *length {
                continue;
            }
            args.extend(["-i".into(), file.to_string_lossy().into_owned()]);
            let label = format!("voice{index}");
            let offset = ((start + voice.narration_offset.max(0.0)) * 1000.0).round() as u64;
            audio_filters.push(format!("[{audio_index}:a]volume={},atrim=duration={},adelay={offset}:all=1,apad,atrim=duration={duration}[{label}]", voice.narration_volume.clamp(0.0,2.0),length-voice.narration_offset.max(0.0)));
            labels.push(label);
            audio_index += 1;
        }
    }
    let mut video = vec![
        "scale=1920:1080:force_original_aspect_ratio=decrease".into(),
        "pad=1920:1080:(ow-iw)/2:(oh-ih)/2".into(),
        "setsar=1".into(),
        format!("fps={fps}"),
    ];
    match edit.theme.as_str() {
        "terminal" => video.push(format!(
            "drawbox=x=0:y=0:w=iw:h=6:color=0x67cf95:t=fill:enable='lt(t,{body_duration})'"
        )),
        "paper" => video.push(format!(
            "drawbox=x=0:y=ih-8:w=iw:h=8:color=0xe8dfc8:t=fill:enable='lt(t,{body_duration})'"
        )),
        "cinema" => {
            video.push(format!(
                "drawbox=x=0:y=0:w=iw:h=60:color=black:t=fill:enable='lt(t,{body_duration})'"
            ));
            video.push(format!(
                "drawbox=x=0:y=ih-60:w=iw:h=60:color=black:t=fill:enable='lt(t,{body_duration})'"
            ));
            video.push("fade=t=in:st=0:d=0.3".into());
        }
        _ => {}
    }
    video.push(format!("ass=filename='{}'", filter_path(&titles)));
    let mut filters = if audio_only {
        vec![]
    } else if gif {
        vec![format!(
            "[0:v]{},fps=12,scale=960:-1,split[g1][g2];[g1]palettegen[palette];[g2][palette]paletteuse[video]",
            video.join(",")
        )]
    } else {
        vec![format!("[0:v]{}[video]", video.join(","))]
    };
    filters.extend(audio_filters);
    if !labels.is_empty() {
        filters.push(format!(
            "{}amix=inputs={}:duration=longest:normalize=0,alimiter=limit=0.95[audio]",
            labels
                .iter()
                .map(|label| format!("[{label}]"))
                .collect::<String>(),
            labels.len()
        ));
    }
    anyhow::ensure!(
        !audio_only || !labels.is_empty(),
        "音声がありません。BGMを選ぶか、アフレコを録音してください。"
    );
    if gif {
        filters.retain(|filter| {
            !filter.contains("[audio]") && !filter.contains("[bgm]") && !filter.contains("[voice")
        });
    }
    args.extend(["-filter_complex".into(), filters.join(";")]);
    if !audio_only {
        args.extend(["-map".into(), "[video]".into()]);
    }
    if !gif && !labels.is_empty() {
        let codec = match format.as_str() {
            "wav" => "pcm_s16le",
            "mp3" => "libmp3lame",
            "flac" => "flac",
            "webm" | "ogg" => "libopus",
            _ => "aac",
        };
        args.extend(
            ["-map", "[audio]", "-c:a", codec, "-ar", "48000", "-ac", "2"]
                .into_iter()
                .map(str::to_owned),
        );
        if !["wav", "flac"].contains(&format.as_str()) {
            args.extend(["-b:a".into(), "192k".into()]);
        }
    }
    if !audio_only && !gif {
        if format == "webm" {
            // The native Windows ARM64 FFmpeg distribution uses SVT-AV1;
            // WebM supports both AV1 and the VP9 used by the other builds.
            let video_options = if cfg!(all(windows, target_arch = "aarch64")) {
                vec!["-c:v", "libsvtav1", "-crf", "32", "-preset", "12"]
            } else {
                vec![
                    "-c:v",
                    "libvpx-vp9",
                    "-crf",
                    "32",
                    "-b:v",
                    "0",
                    "-deadline",
                    "realtime",
                    "-cpu-used",
                    "6",
                ]
            };
            args.extend(video_options.into_iter().map(str::to_owned));
        } else {
            args.extend(
                ["-c:v", "libx264", "-preset", "fast", "-crf", "20"]
                    .into_iter()
                    .map(str::to_owned),
            );
        }
        args.extend(["-pix_fmt".into(), "yuv420p".into()]);
    }
    if ["mp4", "mov", "m4a"].contains(&format.as_str()) {
        args.extend(["-movflags".into(), "+faststart".into()]);
    }
    if gif {
        args.extend(["-loop".into(), "0".into()]);
    }
    args.extend([
        "-t".into(),
        duration.to_string(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
        output.to_string_lossy().into_owned(),
    ]);
    let mut child = hidden_command(&ffmpeg())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(
            "FFmpegを起動できません。ffmpeg.exeをアプリと同じフォルダーに配置してください。",
        )?;
    let stderr = child.stderr.take().unwrap();
    let errors = std::thread::spawn(move || {
        let mut lines = VecDeque::new();
        for line in BufReader::new(stderr)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if lines.len() == 35 {
                lines.pop_front();
            }
            lines.push_back(line);
        }
        lines.into_iter().collect::<Vec<_>>().join("\n")
    });
    for line in BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map_while(std::result::Result::ok)
    {
        if let Some(value) = line
            .strip_prefix("out_time_us=")
            .and_then(|value| value.parse::<f64>().ok())
        {
            progress(((value / 1_000_000.0 / duration * 100.0) as u32).min(99));
        }
    }
    let status = child.wait()?;
    let errors = errors.join().unwrap_or_default();
    anyhow::ensure!(status.success(), "動画生成に失敗しました。\n{errors}");
    output.persist(destination)?;
    progress(100);
    Ok(ExportResult {
        path: destination.to_owned(),
        duration,
        body_duration,
    })
}
pub fn decode_voice(file: &Path, destination: &Path) -> Result<()> {
    let output = hidden_command(&ffmpeg())
        .args(["-y", "-i"])
        .arg(file)
        .args(["-ac", "2", "-ar", "48000"])
        .arg(destination)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "録音のプレビューを読み込めません。"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ass_injection_is_removed() {
        assert_eq!(ass_text("{\\pos(1,2)}秘密"), "＼pos(1,2)秘密");
        assert_eq!(ass_time(96.23), "0:01:36.23");
    }
}
