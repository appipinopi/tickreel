use chrono::{DateTime, Datelike, Local, NaiveDate, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Edit {
    pub theme: String,
    pub bgm: String,
    pub stats: bool,
    pub captions: Vec<Caption>,
    pub narration_offset: f64,
    pub narration_volume: f32,
    pub bgm_volume: f32,
}
impl Default for Edit {
    fn default() -> Self {
        Self {
            theme: "paper".into(),
            bgm: "auto".into(),
            stats: true,
            captions: vec![],
            narration_offset: 0.0,
            narration_volume: 1.0,
            bgm_volume: 0.18,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Caption {
    pub start: f64,
    pub duration: f64,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub color: String,
}
impl Default for Caption {
    fn default() -> Self {
        Self {
            start: 0.0,
            duration: 5.0,
            text: String::new(),
            x: 0.5,
            y: 0.88,
            size: 46.0,
            color: "#ffffff".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Recording {
    pub id: String,
    pub title: String,
    pub source_id: String,
    pub source_ids: Vec<String>,
    pub source_name: String,
    pub layout: String,
    pub protected_apps: Vec<String>,
    pub idle_threshold: u32,
    pub idle_speed: u32,
    pub category: String,
    pub interval: u32,
    pub fps: u32,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub frames: u32,
    pub active_ms: u64,
    pub idle_ms: u64,
    pub bytes: u64,
    pub edit: Edit,
    pub narration_path: Option<String>,
    pub bgm_path: Option<String>,
    pub bgm_name: Option<String>,
    pub export_path: Option<String>,
    pub exported_at: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl Default for Recording {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: "今日の作業".into(),
            source_id: String::new(),
            source_ids: vec![],
            source_name: String::new(),
            layout: "grid".into(),
            protected_apps: vec![],
            idle_threshold: 120,
            idle_speed: 8,
            category: "drawing".into(),
            interval: 10,
            fps: 30,
            started_at: Utc::now().to_rfc3339(),
            ended_at: None,
            status: "recording".into(),
            frames: 0,
            active_ms: 0,
            idle_ms: 0,
            bytes: 0,
            edit: Edit::default(),
            narration_path: None,
            bgm_path: None,
            bgm_name: None,
            export_path: None,
            exported_at: None,
            extra: BTreeMap::new(),
        }
    }
}
impl Recording {
    pub fn local_day(&self) -> String {
        DateTime::parse_from_rfc3339(&self.started_at)
            .map(|date| date.with_timezone(&Local).date_naive().to_string())
            .unwrap_or_else(|_| self.started_at.get(..10).unwrap_or("").to_owned())
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            [1, 5, 10, 30, 60].contains(&self.interval) && [15, 24, 30, 60].contains(&self.fps),
            "撮影間隔・fpsを確認してください。"
        );
        anyhow::ensure!(
            !self.source_ids.is_empty() && self.source_ids.len() <= 8,
            "画面を1〜8枚選択してください。"
        );
        anyhow::ensure!(
            [1, 4, 8, 16].contains(&self.idle_speed)
                && [60, 120, 300].contains(&self.idle_threshold),
            "無操作設定を確認してください。"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameMeta {
    pub index: u32,
    pub captured_at: String,
    pub idle: bool,
}
pub fn selected_frames(recording: &Recording, timeline: &[FrameMeta]) -> Vec<u32> {
    let idle: BTreeMap<_, _> = timeline
        .iter()
        .map(|frame| (frame.index, frame.idle))
        .collect();
    let speed = if [1, 4, 8, 16].contains(&recording.idle_speed) {
        recording.idle_speed
    } else {
        1
    };
    let mut consecutive = 0;
    let mut output = vec![];
    for index in 0..recording.frames {
        if !idle.get(&index).copied().unwrap_or(false) {
            consecutive = 0;
            output.push(index);
        } else {
            if consecutive % speed == 0 || index == recording.frames - 1 {
                output.push(index);
            }
            consecutive += 1;
        }
    }
    output
}
pub fn parse_captions(text: &str) -> anyhow::Result<Vec<Caption>> {
    fn clock(value: &str) -> anyhow::Result<f64> {
        let fields: Vec<_> = value.trim().split(':').collect();
        anyhow::ensure!(fields.len() <= 3, "時刻の形式を確認してください。");
        let mut seconds = 0.0;
        for field in fields {
            let part: f64 = field.parse()?;
            anyhow::ensure!(
                part.is_finite() && part >= 0.0,
                "時刻は0以上で指定してください。"
            );
            seconds = seconds * 60.0 + part;
        }
        Ok(seconds)
    }
    let mut output = vec![];
    for (index, line) in text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        let (range, content) = line.split_once('|').ok_or_else(|| {
            anyhow::anyhow!("テロップ{}行目を「0:00 | 内容」にしてください。", index + 1)
        })?;
        let (start, end) = if let Some((start, end)) = range.split_once('-') {
            (clock(start)?, clock(end)?)
        } else {
            let start = clock(range)?;
            (start, start + 5.0)
        };
        anyhow::ensure!(
            end > start && end - start <= 60.0 && !content.trim().is_empty(),
            "テロップの表示時間は0〜60秒、内容は空白以外にしてください。"
        );
        anyhow::ensure!(
            output.len() < 200 && content.chars().count() <= 250,
            "テロップは200行、1行250文字までです。"
        );
        output.push(Caption {
            start,
            duration: end - start,
            text: content.trim().into(),
            ..Caption::default()
        });
    }
    Ok(output)
}
pub fn period_records(records: &[Recording], date: NaiveDate, month: bool) -> Vec<Recording> {
    let start = if month {
        date.with_day(1).unwrap()
    } else {
        date - TimeDelta::days(date.weekday().num_days_from_monday() as i64)
    };
    let end = if month {
        if start.month() == 12 {
            NaiveDate::from_ymd_opt(start.year() + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1).unwrap()
        }
    } else {
        start + TimeDelta::days(7)
    };
    let mut selected: Vec<_> = records
        .iter()
        .filter(|record| {
            record.frames > 0
                && !["recording", "paused"].contains(&record.status.as_str())
                && DateTime::parse_from_rfc3339(&record.started_at).is_ok_and(|instant| {
                    let day = instant.with_timezone(&Local).date_naive();
                    day >= start && day < end
                })
        })
        .cloned()
        .collect();
    selected.sort_by_key(|record| {
        DateTime::parse_from_rfc3339(&record.started_at)
            .map(|date| date.timestamp_millis())
            .unwrap_or_default()
    });
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_minute_becomes_four_seconds() {
        let record = Recording {
            interval: 1,
            fps: 15,
            idle_speed: 1,
            frames: 60,
            source_ids: vec!["screen:0:0".into()],
            ..Default::default()
        };
        record.validate().unwrap();
        let restored: Recording =
            serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
        assert_eq!(
            selected_frames(&restored, &[]).len() as f64 / restored.fps as f64,
            4.0
        );
    }
    #[test]
    fn idle_keeps_transitions() {
        let record = Recording {
            frames: 20,
            idle_speed: 8,
            ..Default::default()
        };
        let frames: Vec<_> = (0..20)
            .map(|index| FrameMeta {
                index,
                idle: (3..=18).contains(&index),
                ..Default::default()
            })
            .collect();
        assert_eq!(selected_frames(&record, &frames), vec![0, 1, 2, 3, 11, 19]);
    }
    #[test]
    fn captions_validate_and_parse() {
        let captions = parse_captions("0:05-0:12 | 下描き\n30 | 仕上げ").unwrap();
        assert_eq!(captions[0].start, 5.0);
        assert_eq!(captions[0].duration, 7.0);
        assert!(parse_captions("0:12-0:05 | 逆転").is_err());
        assert!(parse_captions("nan | 不正").is_err());
    }
    #[test]
    fn legacy_defaults_load() {
        let record: Recording = serde_json::from_str(r#"{"id":"d006a925-85a9-48d1-a763-2c4663287c36","title":"旧版","frames":5,"activeMs":123,"sourceId":"screen:0:0"}"#).unwrap();
        assert_eq!(record.active_ms, 123);
        assert!(record.edit.stats);
    }
}
