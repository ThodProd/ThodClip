use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub capture_enabled: bool,
    pub honor_clipboard_exclusions: bool,
    pub dedupe_seconds: i64,
    pub max_clips: i64,
    pub max_text_bytes: usize,
    pub max_screenshot_bytes: i64,
    pub auto_import_screenshots: bool,
    pub run_in_background: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            capture_enabled: true,
            honor_clipboard_exclusions: true,
            dedupe_seconds: 3,
            max_clips: 0,
            max_text_bytes: 1024 * 1024,
            max_screenshot_bytes: 0,
            auto_import_screenshots: true,
            run_in_background: false,
        }
    }
}

impl Settings {
    pub fn sanitize(&mut self) {
        self.dedupe_seconds = self.dedupe_seconds.clamp(0, 60);
        self.max_clips = self.max_clips.clamp(0, 200_000);
        self.max_text_bytes = self.max_text_bytes.clamp(64 * 1024, 2 * 1024 * 1024);
        self.max_screenshot_bytes = self.max_screenshot_bytes.clamp(0, 50 * 1024 * 1024 * 1024);
    }

    pub fn load(root: &Path) -> Self {
        let mut settings: Self = fs::read(root.join("config.json"))
            .ok()
            .and_then(|bytes| {
                serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes))
                    .ok()
            })
            .unwrap_or_default();
        settings.sanitize();
        settings
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        fs::create_dir_all(root)?;
        let mut file = tempfile::NamedTempFile::new_in(root)?;
        file.write_all(&serde_json::to_vec_pretty(self)?)?;
        file.persist(root.join("config.json"))?;
        Ok(())
    }
}
