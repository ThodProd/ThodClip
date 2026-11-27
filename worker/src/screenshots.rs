use crate::images;
use base64::{Engine, engine::general_purpose::STANDARD};
use regex::Regex;
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    sync::LazyLock,
    thread,
    time::Duration,
};

static SCREENSHOT_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)screenshot|screen[\s_-]?(shot|clip|snip)|snip|sketch|снимок|скрин|print[\s_-]?screen|screencap|capture").unwrap()
});
static HTML_IMAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?is)<img\b[^>]*\bsrc\s*=\s*(?:"([^"]*)"|'([^']*)')"#).unwrap());
static FILE_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)\b(?:src|href)\s*=\s*(?:"(file:///[^"]*)"|'(file:///[^']*)')"#).unwrap()
});
static HIDDEN_HTML: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script\b.*?</script\s*>|<style\b.*?</style\s*>").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

pub fn flag(name: &str) -> bool {
    env::var(name).is_ok_and(|value| value == "1")
}

pub fn local_app_data() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
}

fn profile() -> PathBuf {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
}

fn unique_directories(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter_map(|path| std::path::absolute(path).ok())
        .filter(|path| path.is_dir() && seen.insert(path.to_string_lossy().to_lowercase()))
        .collect()
}

fn extra_dirs() -> Vec<PathBuf> {
    unique_directories(
        env::var("THODCLIP_SCREENSHOT_DIRS")
            .unwrap_or_default()
            .split(';')
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(PathBuf::from),
    )
}

fn packages() -> impl Iterator<Item = (String, PathBuf)> {
    fs::read_dir(local_app_data().join("Packages"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|item| {
            (
                item.file_name().to_string_lossy().to_lowercase(),
                item.path(),
            )
        })
}

pub fn known_dirs() -> Vec<PathBuf> {
    if flag("THODCLIP_SCREENSHOT_ONLY") {
        return extra_dirs();
    }
    let profile = profile();
    let mut directories = [
        "Pictures/Screenshots",
        "OneDrive/Pictures/Screenshots",
        "Desktop",
        "Pictures",
        "OneDrive/Pictures",
        "Downloads",
    ]
    .map(|path| profile.join(path))
    .to_vec();
    for (name, package) in packages() {
        let temp = package.join("TempState");
        if name.starts_with("microsoft.screensketch_")
            || name.starts_with("microsoft.snippingtool_")
        {
            directories.push(temp.clone());
        } else if !name.starts_with("microsoftwindows.client.core_") {
            continue;
        }
        directories.push(temp.join("ScreenClip"));
    }
    directories.extend(extra_dirs());
    unique_directories(directories)
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

pub fn inside(directory: &Path, path: &Path) -> bool {
    path_key(path)
        .strip_prefix(path_key(directory).trim_end_matches('/'))
        .is_some_and(|suffix| suffix.starts_with('/'))
}

pub fn screenshot(path: &Path) -> bool {
    if !images::is_image(path) {
        return false;
    }
    if extra_dirs().iter().any(|dir| inside(dir, path)) {
        return true;
    }
    let path = path_key(path);
    let Some((parent, file)) = path.rsplit_once('/') else {
        return SCREENSHOT_NAME.is_match(&path);
    };
    let name = parent.rsplit('/').next().unwrap_or_default();
    matches!(
        name,
        "screenshots" | "screenclip" | "screenclips" | "screensketch"
    ) || parent.contains("/tempstate/screenclip")
        || parent.contains("/tempstate/screensketch")
        || (parent.contains("/tempstate")
            && (parent.contains("/microsoft.screensketch_")
                || parent.contains("/microsoft.snippingtool_")))
        || SCREENSHOT_NAME.is_match(file)
}

pub fn clipboard_screenshot(
    files: &[String],
    source: Option<&str>,
    width: u32,
    height: u32,
    screen: (u32, u32),
) -> bool {
    if flag("THODCLIP_ARCHIVE_ALL_IMAGES") {
        return true;
    }
    if width == 0 || height == 0 {
        return false;
    }
    let owner = source.unwrap_or_default().to_lowercase();
    let hints = [
        "snip",
        "screensketch",
        "screenclip",
        "screenshot",
        "screenpresso",
        "sharex",
        "lightshot",
        "greenshot",
        "flameshot",
        "picpick",
        "gyazo",
    ];
    let (sw, sh) = screen;
    files.iter().any(|path| screenshot(Path::new(path)))
        || hints.iter().any(|hint| owner.contains(hint))
        || (sw > 0
            && sh > 0
            && ((width.abs_diff(sw) <= 2 && height.abs_diff(sh) <= 2)
                || u64::from(width) * u64::from(height)
                    >= u64::from(sw) * u64::from(sh) * 85 / 100))
}

pub fn candidates(directory: &Path) -> Vec<(PathBuf, fs::Metadata)> {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !screenshot(&path) {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            metadata.is_file().then_some((path, metadata))
        })
        .collect()
}

pub fn stable(path: &Path, attempts: usize) -> bool {
    let mut previous = None;
    for _ in 0..attempts {
        let Ok(info) = fs::metadata(path) else {
            return false;
        };
        if info.len() > 0 && previous == Some(info.len()) && fs::File::open(path).is_ok() {
            return true;
        }
        previous = Some(info.len());
        thread::sleep(Duration::from_millis(180));
    }
    false
}

pub fn image_reference(value: &str) -> Option<PathBuf> {
    let text = value.trim().trim_matches(['"', '\'', '<', '>']).trim();
    if text.is_empty() || text.encode_utf16().count() > 1024 || text.contains(['\n', '\r', '\t']) {
        return None;
    }
    let parsed;
    let path = if text
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
    {
        parsed = url::Url::parse(text).ok()?.to_file_path().ok()?;
        parsed.as_path()
    } else {
        if text.contains("://") {
            return None;
        }
        Path::new(text)
    };
    if !images::is_image(path) {
        return None;
    }
    if text.contains(['/', '\\']) && path.is_file() {
        return std::path::absolute(path).ok();
    }
    let name = path.file_name()?;
    let profile = profile();
    let mut directories = vec![env::temp_dir(), local_app_data().join("Temp")];
    directories.extend(
        [
            "Pictures",
            "Desktop",
            "Downloads",
            "OneDrive/Pictures",
            "OneDrive/Рабочий стол",
        ]
        .map(|path| profile.join(path)),
    );
    for (name, package) in packages() {
        if [
            "microsoftwindows.client.core_",
            "microsoft.screensketch_",
            "microsoftwindows.client.cbs_",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
        {
            directories.extend(
                ["TempState/ScreenClip", "TempState", "LocalState"].map(|path| package.join(path)),
            );
        }
    }
    directories.extend(known_dirs());
    unique_directories(directories)
        .into_iter()
        .map(|directory| directory.join(name))
        .find(|path| path.is_file())
}

pub fn image_from_html(html: &str) -> Option<(Vec<u8>, u32, u32, Vec<String>)> {
    if html.len() > 4_000_000 {
        return None;
    }
    let lower = html.to_ascii_lowercase();
    let fragment = match (
        lower.find("<!--startfragment-->"),
        lower.find("<!--endfragment-->"),
    ) {
        (Some(start), Some(end)) if start + 20 <= end => &html[start + 20..end],
        _ => html,
    };
    let without_hidden = HIDDEN_HTML.replace_all(fragment, " ");
    if html_escape::decode_html_entities(&TAG.replace_all(&without_hidden, " "))
        .chars()
        .any(char::is_alphanumeric)
    {
        return None;
    }
    for capture in HTML_IMAGE
        .captures_iter(fragment)
        .chain(FILE_LINK.captures_iter(fragment))
    {
        let source =
            html_escape::decode_html_entities(capture.get(1).or_else(|| capture.get(2))?.as_str());
        let source = source.trim();
        let header = source.split_once(',').filter(|(header, _)| {
            let header = header.to_ascii_lowercase();
            header.starts_with("data:image/") && header.contains(";base64")
        });
        if let Some((_, encoded)) = header {
            if let Ok(image) = STANDARD
                .decode(encoded.split_whitespace().collect::<String>())
                .map_err(anyhow::Error::from)
                .and_then(|bytes| images::Picture::open(&bytes))
                && let Ok(png) = image.png()
                && let Ok((width, height)) = image.dimensions()
            {
                return Some((png, width, height, vec![]));
            }
        } else if let Some(path) = image_reference(source)
            && let Ok((png, width, height)) = images::file_png(&path)
        {
            return Some((
                png,
                width,
                height,
                vec![path.to_string_lossy().into_owned()],
            ));
        }
    }
    None
}
