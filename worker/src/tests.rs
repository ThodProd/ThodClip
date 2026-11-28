use crate::{images, screenshots, settings::Settings, store::Store};
use chrono::{Duration, Local};
use image::{DynamicImage, Rgb, RgbImage};
use rusqlite::{Connection, params};
use serde_json::json;
use std::{fs, path::Path};

fn store(settings: Settings) -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().into(), settings).unwrap();
    (dir, store)
}

fn picture(color: [u8; 3], width: u32, height: u32) -> Vec<u8> {
    let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(width, height, Rgb(color)));
    images::png(&image).unwrap()
}

#[test]
fn identical_text_is_searchable_and_counts_repeats() {
    let (_dir, mut store) = store(Settings::default());
    let id = store.add_text("Тест\r\n  буфера", Some("Editor")).unwrap();
    assert_eq!(
        store
            .add_text("Тест\r\n  буфера", Some("New editor"))
            .unwrap(),
        id
    );
    let (total, found) = store.clips("ТЕСТ", "text", false, None, 0, 50).unwrap();
    assert_eq!(total, 1);
    assert_eq!(found[0].repeat_count, 2);
    assert_eq!(found[0].source.as_deref(), Some("New editor"));
    assert_eq!(found[0].preview, "Тест буфера");
    store.settings.dedupe_seconds = 0;
    assert_ne!(store.add_text("Тест\r\n  буфера", None).unwrap(), id);
}

#[test]
fn pinned_history_is_kept_by_retention_and_optional_clear() {
    let (_dir, mut store) = store(Settings {
        max_clips: 2,
        ..Settings::default()
    });
    let pinned = store.add_text("pinned", None).unwrap();
    assert!(store.pin(pinned, true).unwrap());
    let oldest = store.add_text("older", None).unwrap();
    let latest = store.add_text("latest", None).unwrap();
    assert!(store.clip(oldest).unwrap().is_none());
    assert_eq!(store.clips("", "", true, None, 0, 10).unwrap().0, 1);
    assert_eq!(store.clear_clips(true).unwrap(), 1);
    assert!(store.clip(latest).unwrap().is_none());
    assert!(store.clip(pinned).unwrap().unwrap().pinned);
    assert_eq!(store.delete_clips(&[pinned]).unwrap(), 1);
    assert!(store.clip(pinned).unwrap().is_none());
}

#[test]
fn clipboard_file_lists_and_text_limits_preserve_boundary_and_order() {
    let (_dir, mut store) = store(Settings {
        max_text_bytes: 65537,
        ..Settings::default()
    });
    let alpha = "C:\\Test\\A.txt".to_owned();
    let beta = "C:\\Test\\B.txt".to_owned();
    let first = store
        .add_files(&[alpha.clone(), beta.clone(), alpha.to_lowercase()], None)
        .unwrap();
    assert_eq!(
        store
            .add_files(&[beta.clone(), alpha.clone()], None)
            .unwrap(),
        first
    );
    let clip = store.clip(first).unwrap().unwrap();
    assert_eq!(clip.files, [alpha.clone(), beta.clone()]);
    assert_eq!(clip.text, format!("{alpha}\r\n{beta}"));
    let text = "🙂".repeat(20000);
    let id = store.add_text(&text, None).unwrap();
    let full = store.clip(id).unwrap().unwrap();
    assert_eq!(full.text.len(), 65536);
    assert!(full.truncated);
    let (_, list) = store.clips("", "text", false, None, 0, 1).unwrap();
    assert!(list[0].text_clipped);
    assert_eq!(list[0].text.encode_utf16().count(), 4000);
}

#[test]
fn calendar_groups_legacy_rows_by_local_date() {
    let (dir, mut store) = store(Settings::default());
    let today = Local::now().date_naive();
    let prior = today - Duration::days(1);
    let mut ids = Vec::new();
    for value in ["a", "b", "c"] {
        ids.push(store.add_text(value, None).unwrap());
    }
    let database = Connection::open(dir.path().join("clip.db")).unwrap();
    let time = Local::now() - Duration::days(1);
    database
        .execute(
            "UPDATE clips SET created_ms=? WHERE id=?",
            params![time.timestamp_millis(), ids[0]],
        )
        .unwrap();
    let today_key = today.format("%Y-%m-%d").to_string();
    let prior_key = prior.format("%Y-%m-%d").to_string();
    for (day, count) in [(&today_key, 2), (&prior_key, 1)] {
        let calendar = store.calendar(&day[..7]).unwrap();
        assert!(
            calendar["days"]
                .as_array()
                .unwrap()
                .contains(&json!({"date":day,"count":count}))
        );
        assert_eq!(
            store.clips("", "", false, Some(day), 0, 50).unwrap().0,
            count
        );
    }
}

#[test]
fn shared_image_blobs_are_removed_only_after_last_reference() {
    let (_dir, mut store) = store(Settings {
        dedupe_seconds: 0,
        ..Settings::default()
    });
    let data = picture([25, 95, 123], 100, 80);
    let file = String::from("C:\\Pictures\\original.png");
    let a = store
        .add_image(&data, 100, 80, None, std::slice::from_ref(&file))
        .unwrap();
    let b = store.add_image(&data, 100, 80, None, &[]).unwrap();
    let clip = store.clip(a).unwrap().unwrap();
    assert_eq!(clip.files, [file]);
    let original = store.resolve(clip.blob.as_deref().unwrap()).unwrap();
    let thumb = store.image_thumb(&clip).unwrap();
    assert!(original.is_file() && thumb.is_file());
    store.delete_clips(&[a]).unwrap();
    assert!(original.is_file() && thumb.is_file());
    store.delete_clips(&[b]).unwrap();
    assert!(!original.exists() && !thumb.exists());
}

#[test]
fn screenshot_archive_merges_identical_and_similar_images_and_prunes_oldest() {
    let (_dir, mut store) = store(Settings::default());
    let gray = picture([25, 25, 25], 120, 80);
    let similar = picture([26, 26, 26], 120, 80);
    let white = picture([250, 250, 250], 120, 80);
    let id = store
        .add_shot(&gray, "png", "Clipboard", "Снимок", true)
        .unwrap()
        .unwrap();
    assert_eq!(
        store
            .add_shot(&gray, "png", "Clipboard", "Повтор", true)
            .unwrap()
            .as_ref(),
        Some(&id)
    );
    assert_eq!(
        store
            .add_shot(&similar, "png", "File", "Похожий", true)
            .unwrap()
            .as_ref(),
        Some(&id)
    );
    let shot = store.shot(&id).unwrap().unwrap();
    let path = store.resolve(&shot.relative).unwrap();
    let thumb = store.resolve(&shot.thumb_relative).unwrap();
    assert!(path.exists() && thumb.exists());
    std::thread::sleep(std::time::Duration::from_millis(15));
    store.settings.max_screenshot_bytes = white.len() as i64;
    let second = store
        .add_shot(&white, "png", "File", "Белый", true)
        .unwrap()
        .unwrap();
    assert_ne!(second, id);
    assert_eq!(store.shots("", 0, 50).unwrap().0, 1);
    assert!(!path.exists() && !thumb.exists());
    assert_eq!(store.delete_shots(&[second]).unwrap(), 1);
}

#[test]
fn legacy_image_paths_and_filenames_upgrade_and_archive() {
    let (dir, mut store) = store(Settings::default());
    let shots = dir.path().join("Screenshots");
    fs::create_dir(&shots).unwrap();
    let path = shots.join("ScreenClip.png");
    fs::write(&path, picture([5, 20, 100], 100, 80)).unwrap();
    let path = path.to_string_lossy();
    let file = store.add_files(&[path.to_string()], None).unwrap();
    let reference = store.add_text(&path, Some("Editor")).unwrap();
    store.upgrade_images().unwrap();
    for id in [file, reference] {
        let clip = store.clip(id).unwrap().unwrap();
        assert_eq!(clip.kind, "image");
        assert!(store.image_thumb(&clip).unwrap().is_file());
    }
    assert_eq!(store.shots("", 0, 50).unwrap().0, 1);
}

#[test]
fn settings_roundtrip_bom_and_clamps() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    fs::write(&path, b"broken").unwrap();
    assert_eq!(Settings::load(dir.path()), Settings::default());
    let json = "\u{feff}{\"dedupeSeconds\":99,\"maxClips\":-1,\"maxTextBytes\":32,\"maxScreenshotBytes\":4,\"runInBackground\":true}";
    fs::write(&path, json).unwrap();
    let read = Settings::load(dir.path());
    assert_eq!(
        (
            read.dedupe_seconds,
            read.max_clips,
            read.max_text_bytes,
            read.max_screenshot_bytes
        ),
        (60, 0, 65536, 4)
    );
    assert!(read.run_in_background);
    read.save(dir.path()).unwrap();
    assert_eq!(Settings::load(dir.path()), read);
}

#[test]
fn screenshot_detector_reads_folder_and_file_names() {
    let image = r"C:\Users\test\AppData\Local\Packages\MicrosoftWindows.Client.Core_x\TempState\ScreenClip\{a}.png";
    assert!(screenshots::screenshot(Path::new(image)));
    assert!(screenshots::screenshot(Path::new(
        r"C:\Pictures\Screenshots\Some image.png"
    )));
    assert!(!screenshots::screenshot(Path::new(
        r"C:\Pictures\Screenshots\notes.txt"
    )));
    assert!(!screenshots::screenshot(Path::new(
        r"C:\Pictures\Some image.png"
    )));
}

#[test]
fn html_only_images_are_decoded_but_text_is_retained() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let image = STANDARD.encode(picture([60, 70, 80], 10, 20));
    let tag = format!("<img src=\"data:image/png;base64,{image}\">");
    let html = format!("Version:1\r\n<!--StartFragment-->{tag}<!--EndFragment-->");
    let (_, width, height, files) = screenshots::image_from_html(&html).unwrap();
    assert_eq!((width, height), (10, 20));
    assert!(files.is_empty());
    assert!(screenshots::image_from_html(&format!("<p>Обычный текст</p>{tag}")).is_none());
}
