use crate::model::{FrameMeta, Recording};
use anyhow::{Context, Result};
use image::{RgbImage, codecs::jpeg::JpegEncoder};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Store {
    pub root: PathBuf,
}
impl Store {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }
    pub fn directory(&self, id: &str) -> Result<PathBuf> {
        let uuid = uuid::Uuid::parse_str(id).context("記録IDを確認してください。")?;
        anyhow::ensure!(uuid.to_string() == id, "記録IDを確認してください。");
        Ok(self.root.join(id))
    }
    pub fn save(&self, record: &Recording) -> Result<()> {
        let directory = self.directory(&record.id)?;
        fs::create_dir_all(&directory)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
        temporary.write_all(&serde_json::to_vec_pretty(record)?)?;
        temporary.as_file().sync_all()?;
        temporary.persist(directory.join("recording.json"))?;
        Ok(())
    }
    pub fn get(&self, id: &str) -> Result<Recording> {
        Ok(serde_json::from_slice(&fs::read(
            self.directory(id)?.join("recording.json"),
        )?)?)
    }
    pub fn list(&self) -> Result<Vec<Recording>> {
        let mut output = vec![];
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            if let Ok(record) = self.get(&id) {
                output.push(record);
            }
        }
        output.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        Ok(output)
    }
    /// OBS writes to a separate inbox so desktop crash recovery cannot mark a
    /// live OBS capture as interrupted. Only finalized sessions are moved in.
    pub fn import_obs(&self) -> Result<usize> {
        let Some(parent) = self.root.parent() else {
            return Ok(0);
        };
        let inbox = parent.join("obs-recordings");
        if inbox == self.root || !inbox.is_dir() {
            return Ok(0);
        }
        let source = Store::new(inbox)?;
        let mut imported = 0;
        for record in source.list()? {
            if !["ready", "interrupted"].contains(&record.status.as_str())
                || record.ended_at.is_none()
                || record.frames == 0
            {
                continue;
            }
            let from = source.directory(&record.id)?;
            let to = self.directory(&record.id)?;
            if to.exists() || fs::symlink_metadata(&from)?.file_type().is_symlink() {
                continue;
            }
            fs::rename(from, to)?;
            imported += 1;
        }
        Ok(imported)
    }
    pub fn frame(&self, record: &mut Recording, image: &RgbImage, idle: bool) -> Result<()> {
        let directory = self.directory(&record.id)?;
        let mut jpeg = vec![];
        JpegEncoder::new_with_quality(&mut jpeg, 88).encode_image(image)?;
        fs::write(
            directory.join(format!("frame-{:07}.jpg", record.frames)),
            &jpeg,
        )?;
        let metadata = FrameMeta {
            index: record.frames,
            idle,
            captured_at: chrono::Utc::now().to_rfc3339(),
        };
        writeln!(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(directory.join("timeline.jsonl"))?,
            "{}",
            serde_json::to_string(&metadata)?
        )?;
        record.frames += 1;
        record.bytes += jpeg.len() as u64;
        self.save(record)
    }
    pub fn timeline(&self, id: &str) -> Result<Vec<FrameMeta>> {
        let file = self.directory(id)?.join("timeline.jsonl");
        if !file.exists() {
            return Ok(vec![]);
        }
        Ok(fs::read_to_string(file)?
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect())
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        let directory = self.directory(id)?;
        anyhow::ensure!(
            directory.parent() == Some(self.root.as_path()),
            "削除先を確認してください。"
        );
        anyhow::ensure!(
            !fs::symlink_metadata(&directory)?.file_type().is_symlink(),
            "リンクされた記録は削除できません。"
        );
        fs::remove_dir_all(directory)?;
        Ok(())
    }
    pub fn import_legacy(&self, legacy: &Path) -> Result<usize> {
        if !legacy.exists() || legacy == self.root {
            return Ok(0);
        }
        let source = Store::new(legacy.to_owned())?;
        let mut count = 0;
        for mut record in source.list()? {
            if ["recording", "paused"].contains(&record.status.as_str())
                || self.directory(&record.id)?.exists()
            {
                continue;
            }
            let from = source.directory(&record.id)?;
            let staging = tempfile::tempdir_in(&self.root)?;
            for entry in fs::read_dir(&from)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let name = entry.file_name();
                let text = name.to_string_lossy();
                if text.starts_with("frame-") && text.ends_with(".jpg")
                    || text == "timeline.jsonl"
                    || text.starts_with("narration.")
                    || text.starts_with("bgm.")
                {
                    fs::copy(entry.path(), staging.path().join(&name))?;
                }
            }
            let destination = self.directory(&record.id)?;
            record.narration_path = record
                .narration_path
                .as_ref()
                .and_then(|old| Path::new(old).file_name())
                .filter(|name| staging.path().join(name).is_file())
                .map(|name| destination.join(name).to_string_lossy().into_owned());
            record.bgm_path = record
                .bgm_path
                .as_ref()
                .and_then(|old| Path::new(old).file_name())
                .filter(|name| staging.path().join(name).is_file())
                .map(|name| destination.join(name).to_string_lossy().into_owned());
            fs::write(
                staging.path().join("recording.json"),
                serde_json::to_vec_pretty(&record)?,
            )?;
            fs::rename(staging.path(), &destination)?;
            count += 1;
        }
        Ok(count)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn obs_inbox_imports_only_finalized_sessions_without_copying() {
        let temp = tempfile::tempdir().unwrap();
        let inbox = Store::new(temp.path().join("obs-recordings")).unwrap();
        let desktop = Store::new(temp.path().join("recordings")).unwrap();
        let mut record = Recording::default();
        inbox.save(&record).unwrap();
        inbox
            .frame(&mut record, &RgbImage::new(8, 8), false)
            .unwrap();
        assert_eq!(desktop.import_obs().unwrap(), 0);
        record.status = "ready".into();
        record.ended_at = Some(chrono::Utc::now().to_rfc3339());
        inbox.save(&record).unwrap();
        assert_eq!(desktop.import_obs().unwrap(), 1);
        assert_eq!(desktop.import_obs().unwrap(), 0);
        assert!(!inbox.directory(&record.id).unwrap().exists());
        assert_eq!(desktop.get(&record.id).unwrap().frames, 1);
        assert_eq!(desktop.timeline(&record.id).unwrap().len(), 1);
    }
    #[test]
    fn storage_recovery_and_safe_delete() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().join("records")).unwrap();
        let mut record = Recording::default();
        store.save(&record).unwrap();
        store
            .frame(&mut record, &RgbImage::new(8, 8), true)
            .unwrap();
        assert_eq!(store.get(&record.id).unwrap().frames, 1);
        assert!(store.directory("../outside").is_err());
        store.remove(&record.id).unwrap();
        assert!(store.list().unwrap().is_empty());
    }
}
