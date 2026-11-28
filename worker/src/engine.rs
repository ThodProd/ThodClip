use crate::{
    clipboard::{Clipboard, Kind},
    images,
    integration::{self, Integration, Lease},
    screenshot_watch::ScreenshotWatch,
    screenshots,
    settings::Settings,
    store::Store,
};
use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{Local, Utc};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const MAX_INLINE: usize = 5_500_000;
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub struct RpcError(i32, String);

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.1)
    }
}

impl std::error::Error for RpcError {}

fn rpc_error(code: i32, message: impl Into<String>) -> anyhow::Error {
    RpcError(code, message.into()).into()
}

#[derive(Default, Serialize)]
struct ImportState {
    running: bool,
    scanned: usize,
    imported: usize,
    skipped: usize,
    failed: usize,
    error: Option<String>,
}

pub struct Engine {
    integration: Integration,
    store: Store,
    clipboard: Option<Clipboard>,
    lease: Lease,
    owns_capture: bool,
    background: bool,
    started_at: String,
    last_capture: Option<String>,
    last_shot: Option<String>,
    last_import: Arc<Mutex<ImportState>>,
    cancel: Arc<AtomicBool>,
    import_job: Option<thread::JoinHandle<()>>,
    watcher: ScreenshotWatch,
    config_poll: Instant,
    cross_process: Option<(i64, i64)>,
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
}

fn get<T: DeserializeOwned>(params: &Value, key: &str) -> Option<T> {
    serde_json::from_value(params.get(key)?.clone()).ok()
}

fn text<'a>(parameters: &'a Value, key: &str) -> &'a str {
    parameters[key].as_str().unwrap_or_default()
}

fn image_url(mime: &str, data: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(data))
}

fn thumb(path: &Path, limit: u64) -> Option<String> {
    if fs::metadata(path).ok()?.len() > limit {
        return None;
    }
    Some(image_url("image/jpeg", &fs::read(path).ok()?))
}

fn image_response(path: &Path) -> Result<(String, &'static str, bool, usize)> {
    let bytes = fs::read(path)?;
    if bytes.len() <= MAX_INLINE {
        let mime = images::mime(path);
        Ok((image_url(mime, &bytes), mime, false, bytes.len()))
    } else {
        let preview = images::Picture::open(&bytes)?.jpeg(1920, 1440, 88)?;
        Ok((
            image_url("image/jpeg", &preview),
            "image/jpeg",
            true,
            bytes.len(),
        ))
    }
}

fn label(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

impl Engine {
    pub fn new(integration: Integration, background: bool) -> Result<Self> {
        let settings = Settings::load(&integration.root);
        let store = Store::open(integration.root.clone(), settings)?;
        let clipboard = Clipboard::new()
            .map_err(|error| eprintln!("cannot listen to clipboard: {error:#}"))
            .ok();
        let lease = integration.lease("Capture")?;
        let now = Instant::now();
        Ok(Self {
            integration,
            store,
            clipboard,
            lease,
            owns_capture: false,
            background,
            started_at: timestamp(),
            last_capture: None,
            last_shot: None,
            last_import: Arc::new(Mutex::new(ImportState::default())),
            cancel: Arc::new(AtomicBool::new(false)),
            import_job: None,
            watcher: ScreenshotWatch::new(),
            config_poll: now,
            cross_process: None,
        })
    }

    pub fn maintenance(&self) {
        let root = self.store.root.clone();
        thread::spawn(move || {
            let result = Store::open(root.clone(), Settings::load(&root))
                .and_then(|mut store| store.verify_shots().and_then(|()| store.upgrade_images()));
            if let Err(error) = result {
                eprintln!("history maintenance failed: {error:#}");
            }
        });
    }

    pub fn verify_background(&self) {
        if !self.background
            && self.store.settings.run_in_background
            && let Err(error) = self.integration.apply(true, true)
        {
            eprintln!("cannot verify background mode: {error:#}");
        }
    }

    pub fn tick(&mut self) -> Result<bool> {
        let now = Instant::now();
        if self.background && now >= self.config_poll {
            self.config_poll = now + Duration::from_millis(650);
            let next = Settings::load(&self.integration.root);
            if !next.run_in_background {
                return Ok(false);
            }
            if next != self.store.settings {
                let import =
                    !self.store.settings.auto_import_screenshots && next.auto_import_screenshots;
                self.store.settings = next;
                self.store.prune()?;
                if import && self.owns_capture {
                    self.start_import();
                }
            }
        }
        if !self.owns_capture && self.lease.acquire() {
            self.owns_capture = true;
            if self.store.settings.auto_import_screenshots {
                self.start_import();
            }
        }
        let enabled = self.owns_capture && self.store.settings.capture_enabled;
        if let Some((kind, source)) = self.clipboard.as_ref().and_then(|clipboard| {
            clipboard.poll(enabled, self.store.settings.honor_clipboard_exclusions)
        }) {
            self.capture(kind, source.as_deref())?;
        }
        let ready = self.watcher.poll(&self.store.root, enabled);
        for path in ready {
            let source = path.parent().map(label).unwrap_or_default();
            if self.store.add_shot_file(&path, &source, true)?.is_some() {
                self.last_shot = Some(timestamp());
            }
        }
        Ok(true)
    }

    fn capture(&mut self, kind: Kind, source: Option<&str>) -> Result<()> {
        let id = match kind {
            Kind::Text(text) => self.store.add_text(&text, source)?,
            Kind::Files(files) => self.store.add_files(&files, source)?,
            Kind::Image {
                png,
                width,
                height,
                files,
            } => {
                let id = self.store.add_image(&png, width, height, source, &files)?;
                let source = source.filter(|name| !name.trim().is_empty()).map_or_else(
                    || "Буфер обмена".into(),
                    |source| format!("Буфер обмена · {source}"),
                );
                if self
                    .store
                    .add_shot(&png, "png", &source, "", true)?
                    .is_some()
                {
                    self.last_shot = Some(timestamp());
                }
                id
            }
        };
        if id > 0 {
            self.last_capture = Some(timestamp());
        }
        Ok(())
    }

    fn start_import(&mut self) -> bool {
        {
            let mut state = self.last_import.lock().unwrap();
            if state.running {
                return false;
            }
            state.running = true;
            state.error = None;
        }
        let root = self.store.root.clone();
        let state = Arc::clone(&self.last_import);
        let cancel = Arc::clone(&self.cancel);
        let folders = screenshots::known_dirs();
        self.import_job = Some(thread::spawn(move || {
            let result = import(root, folders, &cancel);
            let mut state = state.lock().unwrap();
            match result {
                Ok(summary) => *state = summary,
                Err(error) => state.error = Some(error.to_string()),
            }
            state.running = false;
        }));
        true
    }

    fn status(&mut self) -> Result<Value> {
        let stats = self.store.stats()?;
        let settings = &self.store.settings;
        let background_running = self.background || self.integration.background_running();
        let data_version = self.store.data_version()?;
        let imported = self.last_import.lock().unwrap();
        let capture_owner = if self.owns_capture && !self.background {
            "hub"
        } else if background_running {
            "background"
        } else {
            "none"
        };
        if let Some((prior_data, prior_shots)) = self.cross_process
            && prior_data != data_version
            && prior_shots != stats["shotCount"].as_i64().unwrap_or_default()
        {
            self.last_shot = Some(timestamp());
        }
        self.cross_process = Some((
            data_version,
            stats["shotCount"].as_i64().unwrap_or_default(),
        ));
        Ok(json!({
            "version":VERSION,"hubApi":1,"dataDir":self.store.root,"startedAt":self.started_at,
            "captureEnabled":settings.capture_enabled,
            "captureActive":settings.capture_enabled && ((self.owns_capture && self.clipboard.is_some()) || background_running),
            "captureOwner":capture_owner,"runInBackground":settings.run_in_background,"backgroundRunning":background_running,
            "honorExclusions":settings.honor_clipboard_exclusions,"dedupeSeconds":settings.dedupe_seconds,"folders":screenshots::known_dirs(),
            "clipCount":stats["clipCount"],"shotCount":stats["shotCount"],"clipBytes":stats["clipBytes"],"shotBytes":stats["shotBytes"],
            "databaseBytes":stats["databaseBytes"],"storageBytes":stats["totalBytes"],"revision":self.store.revision + data_version as u64,
            "lastCaptureAt":self.last_capture,"lastShotAt":self.last_shot,"import":*imported,
            "limits":{"maxClips":settings.max_clips,"maxTextBytes":settings.max_text_bytes,"maxScreenshotBytes":settings.max_screenshot_bytes},
        }))
    }

    fn update_settings(&mut self, params: &Value) -> Result<Value> {
        if params.is_null() {
            return Err(rpc_error(-32602, "Нет настроек."));
        }
        let mut next = self.store.settings.clone();
        for (name, field) in [
            ("captureEnabled", &mut next.capture_enabled),
            (
                "honorClipboardExclusions",
                &mut next.honor_clipboard_exclusions,
            ),
            ("autoImportScreenshots", &mut next.auto_import_screenshots),
            ("runInBackground", &mut next.run_in_background),
        ] {
            if let Some(value) = params.get(name) {
                *field = value == true;
            }
        }
        if let Some(value) = get::<i32>(params, "dedupeSeconds") {
            next.dedupe_seconds = value.into();
        }
        if let Some(value) = get::<i32>(params, "maxClips") {
            next.max_clips = value.into();
        }
        if let Some(value) = get::<i32>(params, "maxTextBytes") {
            next.max_text_bytes = value.max(0) as usize;
        }
        if let Some(value) = get::<i64>(params, "maxScreenshotBytes") {
            next.max_screenshot_bytes = value;
        }
        next.sanitize();
        let background_changed = next.run_in_background != self.store.settings.run_in_background;
        next.save(&self.store.root)?;
        self.store.settings = next;
        self.store.prune()?;
        if background_changed
            && let Err(error) = self
                .integration
                .apply(self.store.settings.run_in_background, true)
        {
            if self.store.settings.run_in_background {
                self.store.settings.run_in_background = false;
                self.store.settings.save(&self.store.root)?;
                let _ = self.integration.apply(false, false);
            }
            return Err(rpc_error(
                -32000,
                format!("Не удалось настроить фоновый режим: {error}"),
            ));
        }
        Ok(self.settings())
    }

    fn settings(&self) -> Value {
        json!({"settings":self.store.settings,"dataDir":self.store.root})
    }

    fn clipboard(&mut self) -> Result<&mut Clipboard> {
        self.clipboard
            .as_mut()
            .ok_or_else(|| anyhow!("Буфер обмена недоступен."))
    }

    pub fn handle(&mut self, method: &str, p: &Value) -> Result<Value> {
        let id = get::<i64>(p, "id").unwrap_or_default();
        let shot_id = p["id"].as_str();
        let query = text(p, "query");
        let offset = get::<i64>(p, "offset").unwrap_or(0).max(0);
        let limit = |default| get::<i64>(p, "limit").unwrap_or(default).clamp(1, 200);
        let ok = || json!({"ok":true});
        let clip_missing = || rpc_error(-32004, "Запись не найдена.");
        let shot_missing = || rpc_error(-32004, "Снимок не найден.");
        let shot_path = |store: &Store, relative: &str| {
            let path = store.resolve(relative)?;
            if !path.is_file() {
                return Err(rpc_error(-32004, "Файл снимка не найден."));
            }
            Ok(path)
        };
        let blob_path = |store: &Store, relative: Option<&str>| {
            let path = store.resolve(
                relative.ok_or_else(|| rpc_error(-32004, "Файл изображения не найден."))?,
            )?;
            if !path.is_file() {
                return Err(rpc_error(-32004, "Файл изображения не найден."));
            }
            Ok(path)
        };
        match method {
            "status" => self.status(),
            "app.info" => Ok(
                json!({"id":"thodclip","name":"ThodClip","version":VERSION,"hubApi":1,"dataDir":self.store.root}),
            ),
            "settings.get" => Ok(self.settings()),
            "settings.set" => self.update_settings(p),
            "capture.set" => {
                let enabled = get(p, "enabled").unwrap_or(self.store.settings.capture_enabled);
                self.store.settings.capture_enabled = enabled;
                self.store.settings.save(&self.store.root)?;
                Ok(json!({"captureEnabled":enabled}))
            }
            "clips.calendar" => {
                let month = text(p, "month");
                let default_month = Local::now().format("%Y-%m").to_string();
                let month = if month.trim().is_empty() {
                    &default_month
                } else {
                    month
                };
                if month.len() != 7
                    || chrono::NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d")
                        .is_err()
                {
                    return Err(rpc_error(-32602, "Месяц должен быть в формате ГГГГ-ММ."));
                }
                self.store.calendar(month)
            }
            "clips.list" => {
                let date = p["date"].as_str();
                let limit = limit(40);
                let (total, mut items) = self.store.clips(
                    query,
                    text(p, "kind"),
                    get(p, "pinned").unwrap_or(false),
                    date,
                    offset,
                    limit,
                )?;
                let mut file_budget = 12;
                let mut budget = 5_700_000usize;
                for item in &mut items {
                    if item.kind == "image" {
                        item.thumb = self
                            .store
                            .image_thumb(item)
                            .and_then(|path| thumb(&path, 2_000_000));
                    } else if item.kind == "files" && file_budget > 0 {
                        item.thumb = item.files.iter().find_map(|path| {
                            self.store
                                .file_thumb(Path::new(path))
                                .and_then(|thumb_path| thumb(&thumb_path, 1_500_000))
                        });
                        if item.thumb.is_some() {
                            file_budget -= 1;
                        }
                    }
                    fit_thumb(&mut item.thumb, &mut budget);
                }
                Ok(
                    json!({"revision":self.store.revision+self.store.data_version()? as u64,"total":total,"offset":offset,"limit":limit,"items":items,"date":date}),
                )
            }
            "clips.get" => {
                let clip = self.store.clip(id)?.ok_or_else(clip_missing)?;
                let path = if clip.kind == "image" {
                    Some(blob_path(&self.store, clip.blob.as_deref())?)
                } else if clip.kind == "files" {
                    clip.files
                        .iter()
                        .map(PathBuf::from)
                        .find(|path| images::is_image(path) && path.is_file())
                } else {
                    None
                };
                if let Some(path) = path {
                    let (data, mime, preview, _) = image_response(&path)?;
                    Ok(json!({"item":clip,"dataUrl":data,"mime":mime,"preview":preview}))
                } else {
                    Ok(json!({"item":clip,"preview":false}))
                }
            }
            "clips.copy" => {
                let clip = self.store.clip(id)?.ok_or_else(clip_missing)?;
                let mode = p["mode"].as_str().unwrap_or("default");
                match clip.kind.as_str() {
                    "text" => self.clipboard()?.text(&clip.text)?,
                    "files" if mode == "paths" => self.clipboard()?.text(&clip.text)?,
                    "files" => {
                        let missing = clip
                            .files
                            .iter()
                            .filter(|file| !Path::new(file).exists())
                            .count();
                        if missing > 0 {
                            return Err(rpc_error(
                                -32010,
                                format!(
                                    "Файлы перемещены или удалены ({missing}). Скопируйте пути."
                                ),
                            ));
                        }
                        self.clipboard()?.files(&clip.files)?;
                    }
                    "image" => {
                        let path = blob_path(&self.store, clip.blob.as_deref())?;
                        self.clipboard()?.image(&path)?;
                    }
                    _ => return Err(rpc_error(-32010, "Этот тип записи нельзя скопировать.")),
                }
                Ok(json!({"ok":true,"kind":clip.kind,"mode":mode}))
            }
            "clips.reveal" => {
                let clip = self.store.clip(id)?.ok_or_else(clip_missing)?;
                let path = if clip.kind == "image" {
                    clip.blob
                        .as_deref()
                        .and_then(|blob| self.store.resolve(blob).ok())
                        .filter(|path| path.is_file())
                } else {
                    clip.files
                        .iter()
                        .map(PathBuf::from)
                        .find(|path| path.is_file())
                };
                integration::reveal(
                    &path
                        .ok_or_else(|| rpc_error(-32010, "У этой записи нет доступного файла."))?,
                    true,
                )?;
                Ok(ok())
            }
            "clips.pin" => {
                let pinned = get(p, "pinned").unwrap_or(false);
                if !self.store.pin(id, pinned)? {
                    return Err(clip_missing());
                }
                Ok(json!({"ok":true,"pinned":pinned}))
            }
            "clips.delete" => {
                let mut ids = p["ids"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_i64)
                    .filter(|&id| id > 0)
                    .collect::<Vec<_>>();
                if id > 0 {
                    ids.push(id);
                }
                if ids.is_empty() {
                    return Err(rpc_error(-32602, "Не выбраны записи."));
                }
                Ok(json!({"deleted":self.store.delete_clips(&ids)?}))
            }
            "clips.clear" => {
                let keep = get(p, "keepPinned").unwrap_or(true);
                Ok(json!({"deleted":self.store.clear_clips(keep)?,"keepPinned":keep}))
            }
            "shots.list" => {
                let limit = limit(60);
                let (total, mut items) = self.store.shots(query, offset, limit)?;
                let mut budget = 6_500_000;
                for item in &mut items {
                    item.thumb = self
                        .store
                        .resolve(&item.thumb_relative)
                        .ok()
                        .and_then(|path| thumb(&path, 1_500_000));
                    fit_thumb(&mut item.thumb, &mut budget);
                }
                Ok(
                    json!({"revision":self.store.revision+self.store.data_version()? as u64,"total":total,"offset":offset,"limit":limit,"items":items}),
                )
            }
            "storage.reveal" | "shots.reveal"
                if method == "storage.reveal" || shot_id.is_none_or(|id| id.trim().is_empty()) =>
            {
                integration::reveal(&self.store.root, false)?;
                Ok(json!({"ok":true,"path":self.store.root}))
            }
            "shots.get" | "shots.copy" | "shots.reveal" => {
                let id = shot_id.ok_or_else(|| rpc_error(-32602, "Не указан снимок."))?;
                let item = self.store.shot(id)?.ok_or_else(shot_missing)?;
                let path = shot_path(&self.store, &item.relative)?;
                match method {
                    "shots.get" => {
                        let (data, mime, preview, len) = image_response(&path)?;
                        Ok(
                            json!({"item":item,"dataUrl":data,"mime":mime,"preview":preview,"bytes":len}),
                        )
                    }
                    "shots.copy" => {
                        self.clipboard()?.image(&path)?;
                        Ok(ok())
                    }
                    _ => {
                        integration::reveal(&path, true)?;
                        Ok(ok())
                    }
                }
            }
            "shots.delete" => {
                let mut ids = p["ids"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|id| !id.trim().is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                if let Some(id) = shot_id.filter(|id| !id.trim().is_empty()) {
                    ids.push(id.into());
                }
                if ids.is_empty() {
                    return Err(rpc_error(-32602, "Не выбраны снимки."));
                }
                Ok(json!({"deleted":self.store.delete_shots(&ids)?}))
            }
            "shots.clear" => {
                self.last_shot = None;
                Ok(json!({"deleted":self.store.clear_shots()?}))
            }
            "shots.import" => {
                let started = self.start_import();
                let message = if started {
                    "Импорт запущен."
                } else {
                    "Импорт уже выполняется."
                };
                Ok(json!({"started":started,"message":message}))
            }
            _ => Err(rpc_error(-32601, format!("Неизвестный метод: {method}"))),
        }
    }

    pub fn response(&mut self, line: &str) -> Option<Value> {
        let request = match serde_json::from_str::<Value>(line) {
            Ok(value) if value.is_object() => value,
            _ => {
                return Some(
                    json!({"jsonrpc":"2.0","error":{"code":-32600,"message":"Ожидался объект JSON-RPC."}}),
                );
            }
        };
        let id = &request["id"];
        if !id.is_string() && !id.is_number() {
            return None;
        }
        let result = match request["method"].as_str() {
            Some(method) => self.handle(method, &request["params"]),
            None => Err(rpc_error(-32600, "Не указан метод.")),
        };
        let response = match result {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err(error) => {
                let code = error
                    .downcast_ref::<RpcError>()
                    .map_or(-32000, |error| error.0);
                json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":error.to_string()}})
            }
        };
        Some(response)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(import) = self.import_job.take() {
            let deadline = Instant::now() + Duration::from_millis(1250);
            while !import.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(25));
            }
            if import.is_finished() {
                let _ = import.join();
            }
        }
    }
}

fn fit_thumb(thumb: &mut Option<String>, budget: &mut usize) {
    if let Some(data) = thumb {
        if data.len() > *budget {
            *thumb = None;
        } else {
            *budget -= data.len();
        }
    }
}

fn import(root: PathBuf, folders: Vec<PathBuf>, cancel: &AtomicBool) -> Result<ImportState> {
    let mut store = Store::open(root.clone(), Settings::load(&root))?;
    let before = store.stats()?["shotCount"].as_i64().unwrap_or_default();
    let mut summary = ImportState::default();
    for dir in folders {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut files = screenshots::candidates(&dir);
        files.sort_by_cached_key(|(_, info)| std::cmp::Reverse(info.modified().ok()));
        for (path, _) in files.into_iter().take(5000) {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            if screenshots::inside(&root, &path) {
                continue;
            }
            summary.scanned += 1;
            if !screenshots::stable(&path, 2)
                || store.add_shot_file(&path, &label(&dir), true).is_err()
            {
                summary.failed += 1;
            }
        }
    }
    let after = store.stats()?["shotCount"].as_i64().unwrap_or_default();
    summary.imported = (after - before).max(0) as usize;
    summary.skipped = summary
        .scanned
        .saturating_sub(summary.imported + summary.failed);
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_clipboard_image_is_saved_in_history_and_archive() {
        let directory = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(
            Integration {
                root: directory.path().join("data"),
                home: directory.path().join("cache"),
            },
            false,
        )
        .unwrap();
        let png = images::png(&image::DynamicImage::new_rgb8(64, 48)).unwrap();
        engine
            .capture(
                Kind::Image {
                    png,
                    width: 64,
                    height: 48,
                    files: vec![],
                },
                Some("ThodShot"),
            )
            .unwrap();

        let (total, shots) = engine.store.shots("", 0, 10).unwrap();
        assert_eq!(total, 1);
        assert_eq!(shots[0].source, "Буфер обмена · ThodShot");
        let (total, _) = engine.store.clips("", "image", false, None, 0, 10).unwrap();
        assert_eq!(total, 1);
    }
}
