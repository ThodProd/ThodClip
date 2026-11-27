use crate::{images, settings::Settings};
use anyhow::{Result, ensure};
use chrono::{DateTime, Local, Months, NaiveDate, SecondsFormat, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior::Immediate, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
    time::Duration,
};

const CLIP_COLS: &str = "id,kind,content,files,blob,width,height,bytes,source,created_ms,updated_ms,repeat_count,pinned,truncated";
const SHOT_COLS: &str = "id,relative,thumb,width,height,bytes,source,name,created_ms";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: i64,
    pub kind: String,
    pub text: String,
    pub preview: String,
    pub files: Vec<String>,
    #[serde(skip)]
    pub blob: Option<String>,
    pub width: u32,
    pub height: u32,
    pub bytes: i64,
    pub source: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub repeat_count: i64,
    pub pinned: bool,
    pub truncated: bool,
    pub text_clipped: bool,
    pub thumb: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    pub id: String,
    #[serde(skip)]
    pub relative: String,
    #[serde(skip)]
    pub thumb_relative: String,
    pub width: u32,
    pub height: u32,
    pub bytes: i64,
    pub source: String,
    pub name: String,
    pub created_at: String,
    pub thumb: Option<String>,
}

pub struct Store {
    pub root: PathBuf,
    pub settings: Settings,
    pub revision: u64,
    db: Connection,
}

fn iso(ms: i64) -> String {
    DateTime::from_timestamp_millis(ms)
        .map(|date| date.to_rfc3339_opts(SecondsFormat::Millis, false))
        .unwrap_or_default()
}

fn day_key(ms: i64) -> String {
    DateTime::from_timestamp_millis(ms)
        .map(|date| date.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

fn day_start(day: NaiveDate) -> i64 {
    let midnight = day.and_hms_opt(0, 0, 0).expect("midnight");
    for minutes in 0..180 {
        if let Some(date) = Local
            .from_local_datetime(&(midnight + chrono::Duration::minutes(minutes)))
            .earliest()
        {
            return date.timestamp_millis();
        }
    }
    midnight.and_utc().timestamp_millis()
}

fn preview(text: &str) -> String {
    let mut result = String::new();
    let mut space = false;
    let mut len = 0;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !space {
                result.push(' ');
                len += 1;
            }
            space = true;
            continue;
        }
        result.push(ch);
        len += ch.len_utf16();
        space = false;
        if len >= 700 {
            result.push('…');
            break;
        }
    }
    result.trim().into()
}

fn map_clip(row: &Row<'_>, text_limit: usize) -> rusqlite::Result<Clip> {
    let content: String = row.get(2)?;
    let mut units = 0;
    let clipped_at = content.char_indices().find_map(|(index, ch)| {
        units += ch.len_utf16();
        (units > text_limit).then_some(index)
    });
    let preview = preview(&content);
    let text = match clipped_at {
        Some(index) => content[..index].into(),
        None => content,
    };
    Ok(Clip {
        id: row.get(0)?,
        kind: row.get(1)?,
        text,
        preview,
        files: serde_json::from_str(row.get_ref(3)?.as_str()?).unwrap_or_default(),
        blob: row.get(4)?,
        width: row.get(5)?,
        height: row.get(6)?,
        bytes: row.get(7)?,
        source: row.get(8)?,
        created_at: iso(row.get(9)?),
        updated_at: iso(row.get(10)?),
        repeat_count: row.get(11)?,
        pinned: row.get(12)?,
        truncated: row.get(13)?,
        text_clipped: clipped_at.is_some(),
        thumb: None,
    })
}

fn map_shot(row: &Row<'_>) -> rusqlite::Result<Shot> {
    Ok(Shot {
        id: row.get(0)?,
        relative: row.get(1)?,
        thumb_relative: row.get(2)?,
        width: row.get(3)?,
        height: row.get(4)?,
        bytes: row.get(5)?,
        source: row.get(6)?,
        name: row.get(7)?,
        created_at: iso(row.get(8)?),
        thumb: None,
    })
}

fn unique_files(files: &[String], limit: usize) -> Vec<&str> {
    let mut seen = HashSet::new();
    files
        .iter()
        .map(|path| path.trim())
        .filter(|path| !path.is_empty() && seen.insert(path.to_lowercase()))
        .take(limit)
        .collect()
}

pub fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let directory = path.parent().expect("file parent");
    fs::create_dir_all(directory)?;
    let mut temp = tempfile::NamedTempFile::new_in(directory)?;
    temp.write_all(bytes)?;
    temp.persist(path)?;
    Ok(())
}

impl Store {
    pub fn open(root: PathBuf, settings: Settings) -> Result<Self> {
        for relative in ["blobs", "thumbs", "shots/orig", "shots/thumb"] {
            fs::create_dir_all(root.join(relative))?;
        }
        let db = Connection::open(root.join("clip.db"))?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        db.execute_batch(include_str!("schema.sql"))?;
        Ok(Self {
            root,
            settings,
            db,
            revision: 1,
        })
    }

    pub fn resolve(&self, relative: &str) -> Result<PathBuf> {
        let relative = Path::new(relative);
        ensure!(
            !relative.as_os_str().is_empty()
                && relative
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
            "Путь выходит за пределы каталога данных."
        );
        Ok(self.root.join(relative))
    }

    pub fn image_thumb(&self, clip: &Clip) -> Option<PathBuf> {
        let stem = Path::new(clip.blob.as_deref()?).file_stem()?.to_str()?;
        let path = self.root.join("thumbs").join(format!("{stem}.jpg"));
        path.is_file().then_some(path)
    }

    pub fn file_thumb(&self, path: &Path) -> Option<PathBuf> {
        let metadata = fs::metadata(path).ok()?;
        if !images::is_image(path) || metadata.len() > 96 * 1024 * 1024 {
            return None;
        }
        let ticks = metadata
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_nanos()
            / 100
            + 621355968000000000;
        let key = format!("file\n{}\n{}\n{ticks}", path.display(), metadata.len());
        let target = self
            .root
            .join("thumbs/files")
            .join(format!("{}.jpg", images::hash(key)));
        if !target.is_file() {
            let image = images::Picture::open(&fs::read(path).ok()?).ok()?;
            write_file(&target, &image.jpeg(560, 420, 82).ok()?).ok()?;
        }
        Some(target)
    }

    pub fn add_text(&mut self, text: &str, source: Option<&str>) -> Result<i64> {
        if text.is_empty() {
            return Ok(0);
        }
        let truncated = text.len() > self.settings.max_text_bytes;
        let text = &text[..text.floor_char_boundary(self.settings.max_text_bytes.min(text.len()))];
        let hash = images::hash(format!("text\n{text}"));
        let search = format!("{text}\n{}", source.unwrap_or_default()).to_lowercase();
        self.insert_clip(
            params![
                "text",
                text,
                search,
                "[]",
                None::<String>,
                0,
                0,
                text.len(),
                hash,
                source,
                truncated
            ],
            &hash,
            source,
        )
    }

    pub fn add_files(&mut self, files: &[String], source: Option<&str>) -> Result<i64> {
        let files = unique_files(files, 500);
        if files.is_empty() {
            return Ok(0);
        }
        let content = files.join("\r\n");
        let mut sorted = files.clone();
        sorted.sort_by_key(|path| path.to_lowercase());
        let hash = images::hash(format!("files\n{}", sorted.join("\n")));
        let bytes: usize = files.iter().map(|path| path.len()).sum();
        let search = format!("{content}\n{}", source.unwrap_or_default()).to_lowercase();
        self.insert_clip(
            params![
                "files",
                content,
                search,
                serde_json::to_string(&files)?,
                None::<String>,
                0,
                0,
                bytes,
                hash,
                source,
                false
            ],
            &hash,
            source,
        )
    }

    pub fn add_image(
        &mut self,
        png: &[u8],
        width: u32,
        height: u32,
        source: Option<&str>,
        files: &[String],
    ) -> Result<i64> {
        if png.is_empty() || width == 0 || height == 0 {
            return Ok(0);
        }
        let hash = images::hash(png);
        let relative = self.save_clip_image(png, &hash)?;
        let files = unique_files(files, 20);
        let search = image_search(width, height, source, &files);
        self.insert_clip(
            params![
                "image",
                "",
                search,
                serde_json::to_string(&files)?,
                relative,
                width,
                height,
                png.len(),
                hash,
                source,
                false
            ],
            &hash,
            source,
        )
    }

    fn save_clip_image(&self, png: &[u8], hash: &str) -> Result<String> {
        let relative = format!("blobs/{hash}.png");
        let blob = self.root.join(&relative);
        if !blob.is_file() {
            write_file(&blob, png)?;
        }
        let thumb = self.root.join(format!("thumbs/{hash}.jpg"));
        if !thumb.is_file()
            && let Ok(jpeg) = images::Picture::open(png).and_then(|image| image.jpeg(560, 420, 82))
        {
            let _ = write_file(&thumb, &jpeg);
        }
        Ok(relative)
    }

    fn insert_clip(
        &mut self,
        values: impl rusqlite::Params,
        hash: &str,
        source: Option<&str>,
    ) -> Result<i64> {
        let now = Utc::now().timestamp_millis();
        let tx = self.db.transaction_with_behavior(Immediate)?;
        if self.settings.dedupe_seconds > 0 {
            let existing: Option<(i64, i64)> = tx
                .query_row(
                    "SELECT id,updated_ms FROM clips WHERE hash=? ORDER BY id DESC LIMIT 1",
                    [hash],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((id, updated)) = existing
                && now - updated <= self.settings.dedupe_seconds * 1000
            {
                tx.execute("UPDATE clips SET updated_ms=?,repeat_count=repeat_count+1,source=COALESCE(?,source) WHERE id=?", params![now, source, id])?;
                tx.commit()?;
                self.revision += 1;
                return Ok(id);
            }
        }
        // Use SQLite's clock in the INSERT so the argument order is shared by all three capture kinds.
        tx.execute("INSERT INTO clips(kind,content,search,files,blob,width,height,bytes,hash,source,created_ms,updated_ms,truncated)
                    VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,CAST(unixepoch('subsec')*1000 AS INTEGER),CAST(unixepoch('subsec')*1000 AS INTEGER),?11)", values)?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        self.prune_clips()?;
        self.revision += 1;
        Ok(id)
    }

    pub fn clips(
        &self,
        query: &str,
        kind: &str,
        pinned: bool,
        date: Option<&str>,
        offset: i64,
        limit: i64,
    ) -> Result<(i64, Vec<Clip>)> {
        let day = date.and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok());
        let (from, to) = day
            .and_then(|day| Some((day_start(day), day_start(day.succ_opt()?))))
            .unwrap_or((i64::MIN, i64::MAX));
        let kind = if matches!(kind, "text" | "image" | "files") {
            kind
        } else {
            ""
        };
        let query = query.trim().to_lowercase();
        let filter = "WHERE (?1='' OR kind=?1) AND (?2=0 OR pinned=1) AND (?3='' OR instr(search,?3)>0) AND created_ms>=?4 AND created_ms<?5";
        let total = self.db.query_row(
            &format!("SELECT COUNT(*) FROM clips {filter}"),
            params![kind, pinned, query, from, to],
            |r| r.get(0),
        )?;
        let mut statement = self.db.prepare(&format!("SELECT {CLIP_COLS} FROM clips {filter} ORDER BY pinned DESC, created_ms DESC, id DESC LIMIT ?6 OFFSET ?7"))?;
        let items = statement
            .query_map(params![kind, pinned, query, from, to, limit, offset], |r| {
                map_clip(r, 4000)
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok((total, items))
    }

    pub fn clip(&self, id: i64) -> Result<Option<Clip>> {
        Ok(self
            .db
            .query_row(
                &format!("SELECT {CLIP_COLS} FROM clips WHERE id=?"),
                [id],
                |r| map_clip(r, usize::MAX),
            )
            .optional()?)
    }

    pub fn calendar(&self, month: &str) -> Result<Value> {
        let first = NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d")?;
        let next = first
            .checked_add_months(Months::new(1))
            .ok_or_else(|| anyhow::anyhow!("Месяц должен быть в формате ГГГГ-ММ."))?;
        let mut statement = self
            .db
            .prepare("SELECT created_ms FROM clips WHERE created_ms>=? AND created_ms<?")?;
        let mut days = BTreeMap::new();
        for time in statement.query_map(params![day_start(first), day_start(next)], |r| r.get(0))? {
            *days.entry(day_key(time?)).or_insert(0i64) += 1;
        }
        let (first_date, last_date): (Option<i64>, Option<i64>) = self.db.query_row(
            "SELECT MIN(created_ms),MAX(created_ms) FROM clips",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let total = days.values().sum::<i64>();
        let days = days
            .into_iter()
            .map(|(date, count)| json!({"date":date,"count":count}))
            .collect::<Vec<_>>();
        Ok(
            json!({"month":month,"total":total,"firstDate":first_date.map(day_key),"lastDate":last_date.map(day_key),"days":days}),
        )
    }

    pub fn pin(&mut self, id: i64, pinned: bool) -> Result<bool> {
        let changed = self
            .db
            .execute("UPDATE clips SET pinned=? WHERE id=?", params![pinned, id])?
            > 0;
        self.revision += u64::from(changed);
        Ok(changed)
    }

    pub fn delete_clips(&mut self, ids: &[i64]) -> Result<usize> {
        let ids: HashSet<_> = ids
            .iter()
            .copied()
            .filter(|&id| id > 0)
            .take(1000)
            .collect();
        if ids.is_empty() {
            return Ok(0);
        }
        let tx = self.db.transaction_with_behavior(Immediate)?;
        let mut blobs = Vec::new();
        for id in &ids {
            let blob = tx
                .query_row("DELETE FROM clips WHERE id=? RETURNING blob", [id], |r| {
                    r.get::<_, Option<String>>(0)
                })
                .optional()?
                .flatten();
            blobs.extend(blob);
        }
        tx.commit()?;
        self.remove_unused(blobs)?;
        self.revision += 1;
        Ok(ids.len())
    }

    pub fn clear_clips(&mut self, keep_pinned: bool) -> Result<usize> {
        let (count, blobs) = self.remove_clips_where("(?1=0 OR pinned=0)", keep_pinned)?;
        self.remove_unused(blobs)?;
        self.revision += 1;
        Ok(count)
    }

    fn remove_clips_where(
        &mut self,
        predicate: &str,
        value: impl rusqlite::ToSql,
    ) -> Result<(usize, Vec<String>)> {
        let mut statement = self.db.prepare(&format!(
            "DELETE FROM clips WHERE {predicate} RETURNING blob"
        ))?;
        let entries = statement
            .query_map([value], |r| r.get::<_, Option<String>>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((entries.len(), entries.into_iter().flatten().collect()))
    }

    fn remove_unused(&self, blobs: Vec<String>) -> Result<()> {
        let mut statement = self
            .db
            .prepare("SELECT EXISTS(SELECT 1 FROM clips WHERE blob=?)")?;
        for blob in blobs.iter().collect::<HashSet<_>>() {
            if !statement.query_row([blob], |r| r.get::<_, bool>(0))? {
                self.remove_file(blob);
                if let Some(stem) = Path::new(blob).file_stem().and_then(|s| s.to_str()) {
                    self.remove_file(&format!("thumbs/{stem}.jpg"));
                }
            }
        }
        Ok(())
    }

    fn remove_file(&self, relative: &str) {
        if let Ok(path) = self.resolve(relative) {
            let _ = fs::remove_file(path);
        }
    }

    fn prune_clips(&mut self) -> Result<()> {
        if self.settings.max_clips > 0 {
            let (_, blobs) = self.remove_clips_where("pinned=0 AND id NOT IN (SELECT id FROM clips ORDER BY pinned DESC,created_ms DESC,id DESC LIMIT ?1)", self.settings.max_clips)?;
            self.remove_unused(blobs)?;
        }
        Ok(())
    }

    pub fn prune(&mut self) -> Result<()> {
        self.prune_clips()?;
        self.prune_shots()?;
        self.revision += 1;
        Ok(())
    }

    pub fn add_shot_file(
        &mut self,
        path: &Path,
        source: &str,
        prune: bool,
    ) -> Result<Option<String>> {
        if !images::is_image(path) || !path.is_file() {
            return Ok(None);
        }
        let bytes = fs::read(path)?;
        self.add_shot(
            &bytes,
            &images::extension(path),
            source,
            &path.file_name().unwrap_or_default().to_string_lossy(),
            prune,
        )
    }

    pub fn add_shot(
        &mut self,
        bytes: &[u8],
        extension: &str,
        source: &str,
        name: &str,
        prune: bool,
    ) -> Result<Option<String>> {
        let hash = images::hash(bytes);
        let existing = self
            .db
            .query_row("SELECT id FROM shots WHERE hash=?", [&hash], |r| r.get(0))
            .optional()?;
        if existing.is_some() {
            return Ok(existing);
        }
        let Ok(image) = images::Picture::open(bytes) else {
            return Ok(None);
        };
        let (width, height) = image.dimensions()?;
        let thumb = image.jpeg(560, 420, 82)?;
        let now = Utc::now();
        let fingerprint = images::fingerprint(&thumb)?;
        let recent = {
            let mut statement = self.db.prepare("SELECT id,thumb FROM shots WHERE width=? AND height=? AND created_ms>=? ORDER BY created_ms DESC LIMIT 12")?;
            statement
                .query_map(
                    params![width, height, now.timestamp_millis() - 900_000],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (id, path) in recent {
            if let Ok(other) = self
                .resolve(&path)
                .and_then(|path| Ok(fs::read(path)?))
                .and_then(|bytes| images::fingerprint(&bytes))
            {
                let difference: u32 = fingerprint
                    .iter()
                    .zip(other)
                    .map(|(&a, b)| u32::from(a.abs_diff(b)))
                    .sum();
                if difference <= 4 * fingerprint.len() as u32 {
                    return Ok(Some(id));
                }
            }
        }
        let id = format!("{}-{}", now.format("%Y%m%d-%H%M%S"), &hash[..8]);
        let month = now.format("%Y-%m");
        let relative = format!("shots/orig/{month}/{id}.{extension}");
        let thumb_relative = format!("shots/thumb/{month}/{id}.jpg");
        let name = if name.is_empty() {
            format!("Снимок {width}×{height}")
        } else {
            name.into()
        };
        let search = format!("{name}\n{source}").to_lowercase();
        let original_path = self.resolve(&relative)?;
        let thumb_path = self.resolve(&thumb_relative)?;
        let tx = self.db.transaction_with_behavior(Immediate)?;
        let added = tx.execute("INSERT OR IGNORE INTO shots(id,relative,thumb,width,height,bytes,hash,source,name,search,created_ms) VALUES(?,?,?,?,?,?,?,?,?,?,?)", params![id,relative,thumb_relative,width,height,bytes.len(),hash,source,name,search,now.timestamp_millis()])?;
        if added == 0 {
            return Ok(tx
                .query_row("SELECT id FROM shots WHERE hash=?", [&hash], |r| r.get(0))
                .optional()?);
        }
        if let Err(error) = write_file(&original_path, bytes)
            .and_then(|()| write_file(&thumb_path, &thumb))
            .and_then(|()| Ok(tx.commit()?))
        {
            self.remove_file(&relative);
            self.remove_file(&thumb_relative);
            return Err(error);
        }
        if prune {
            self.prune_shots()?;
        }
        self.revision += 1;
        Ok(Some(id))
    }

    pub fn shots(&self, query: &str, offset: i64, limit: i64) -> Result<(i64, Vec<Shot>)> {
        let query = query.trim().to_lowercase();
        let filter = "WHERE (?1='' OR instr(search,?1)>0)";
        let total = self.db.query_row(
            &format!("SELECT COUNT(*) FROM shots {filter}"),
            [&query],
            |r| r.get(0),
        )?;
        let mut statement = self.db.prepare(&format!("SELECT {SHOT_COLS} FROM shots {filter} ORDER BY created_ms DESC,id DESC LIMIT ?2 OFFSET ?3"))?;
        let items = statement
            .query_map(params![query, limit, offset], map_shot)?
            .collect::<rusqlite::Result<_>>()?;
        Ok((total, items))
    }

    pub fn shot(&self, id: &str) -> Result<Option<Shot>> {
        Ok(self
            .db
            .query_row(
                &format!("SELECT {SHOT_COLS} FROM shots WHERE id=?"),
                [id],
                map_shot,
            )
            .optional()?)
    }

    pub fn shot_hash_exists(&self, hash: &str) -> Result<bool> {
        Ok(self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM shots WHERE hash=?)",
            [hash],
            |r| r.get(0),
        )?)
    }

    pub fn delete_shots(&mut self, ids: &[String]) -> Result<usize> {
        let tx = self.db.transaction_with_behavior(Immediate)?;
        let mut files = Vec::new();
        for id in ids.iter().collect::<HashSet<_>>().into_iter().take(1000) {
            files.extend(
                tx.query_row(
                    "DELETE FROM shots WHERE id=? RETURNING relative,thumb",
                    [id],
                    shot_files,
                )
                .optional()?,
            );
        }
        tx.commit()?;
        for (original, thumb) in &files {
            self.remove_file(original);
            self.remove_file(thumb);
        }
        self.revision += 1;
        Ok(files.len())
    }

    pub fn clear_shots(&mut self) -> Result<usize> {
        let files = self
            .db
            .prepare("DELETE FROM shots RETURNING relative,thumb")?
            .query_map([], shot_files)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (original, thumb) in &files {
            self.remove_file(original);
            self.remove_file(thumb);
        }
        self.revision += 1;
        Ok(files.len())
    }

    fn prune_shots(&mut self) -> Result<()> {
        let limit = self.settings.max_screenshot_bytes;
        if limit == 0 {
            return Ok(());
        }
        let ids = self.db.prepare("SELECT id FROM (SELECT id,SUM(bytes) OVER (ORDER BY created_ms DESC,id DESC) AS kept FROM shots) WHERE kept>?")?
            .query_map([limit], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        for chunk in ids.chunks(1000) {
            self.delete_shots(chunk)?;
        }
        Ok(())
    }

    pub fn verify_shots(&mut self) -> Result<()> {
        let rows = self
            .db
            .prepare("SELECT id,relative,thumb FROM shots")?
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let ids: Vec<_> = rows
            .into_iter()
            .filter_map(|(id, original, thumb)| {
                (!self.resolve(&original).is_ok_and(|p| p.is_file())
                    || !self.resolve(&thumb).is_ok_and(|p| p.is_file()))
                .then_some(id)
            })
            .collect();
        for chunk in ids.chunks(1000) {
            self.delete_shots(chunk)?;
        }
        Ok(())
    }

    pub fn stats(&self) -> Result<Value> {
        let counts = |sql| {
            self.db
                .query_row(sql, [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        };
        let (clip_count, clip_bytes) = counts("SELECT COUNT(*),COALESCE(SUM(bytes),0) FROM clips")?;
        let (shot_count, shot_bytes) = counts("SELECT COUNT(*),COALESCE(SUM(bytes),0) FROM shots")?;
        let db_bytes = ["clip.db", "clip.db-wal", "clip.db-shm"]
            .iter()
            .filter_map(|file| fs::metadata(self.root.join(file)).ok())
            .map(|m| m.len() as i64)
            .sum::<i64>();
        Ok(
            json!({"clipCount":clip_count,"shotCount":shot_count,"clipBytes":clip_bytes,"shotBytes":shot_bytes,"databaseBytes":db_bytes,"totalBytes":clip_bytes+shot_bytes+db_bytes}),
        )
    }
}

fn shot_files(row: &Row<'_>) -> rusqlite::Result<(String, String)> {
    Ok((row.get(0)?, row.get(1)?))
}

fn image_search(width: u32, height: u32, source: Option<&str>, files: &[&str]) -> String {
    format!(
        "изображение {width}×{height}\n{}\n{}",
        source.unwrap_or_default(),
        files.join("\n")
    )
    .to_lowercase()
}
impl Store {
    pub fn data_version(&self) -> Result<i64> {
        Ok(self
            .db
            .query_row("PRAGMA data_version", [], |row| row.get(0))?)
    }

    pub fn upgrade_images(&mut self) -> Result<()> {
        use crate::screenshots::{image_reference, screenshot};
        let files = self.db.prepare("SELECT id,files FROM clips WHERE kind='files' AND blob IS NULL ORDER BY id DESC LIMIT 120")?
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let texts = self.db.prepare("SELECT id,content FROM clips WHERE kind='text' AND blob IS NULL AND length(content) BETWEEN 1 AND 1024 ORDER BY id DESC LIMIT 300")?
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let file_paths = files.into_iter().filter_map(|(id, value)| {
            let paths: Vec<String> = serde_json::from_str(&value).ok()?;
            (paths.len() == 1).then(|| (id, "files", PathBuf::from(&paths[0])))
        });
        let text_paths = texts
            .into_iter()
            .filter_map(|(id, text)| image_reference(&text).map(|path| (id, "text", path)));
        for (id, kind, path) in file_paths.chain(text_paths) {
            if !images::is_image(&path) {
                continue;
            }
            let Ok((png, width, height)) = images::file_png(&path) else {
                continue;
            };
            let hash = images::hash(&png);
            let Ok(blob) = self.save_clip_image(&png, &hash) else {
                continue;
            };
            let path = path.to_string_lossy();
            let search = format!("изображение {width}×{height}\n{path}").to_lowercase();
            let changed = self.db.execute("UPDATE clips SET kind='image',content='',files=?,search=?,blob=?,width=?,height=?,bytes=?,hash=?,truncated=0 WHERE id=? AND kind=?", params![serde_json::to_string(&[path])?,search,blob,width,height,png.len(),hash,id,kind])?;
            self.revision += changed as u64;
        }

        let (_, clips) = self.clips("", "image", false, None, 0, 200)?;
        for clip in clips {
            if !clip.files.iter().any(|file| screenshot(Path::new(file))) {
                continue;
            }
            let Some(path) = clip.blob.and_then(|blob| self.resolve(&blob).ok()) else {
                continue;
            };
            let Ok(png) = fs::read(path) else {
                continue;
            };
            if self.shot_hash_exists(&images::hash(&png))? {
                continue;
            }
            let source = clip
                .source
                .filter(|name| !name.trim().is_empty())
                .map_or_else(
                    || "Буфер обмена".into(),
                    |name| format!("Буфер обмена · {name}"),
                );
            self.add_shot(&png, "png", &source, "", true)?;
        }
        Ok(())
    }
}
