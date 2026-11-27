use crate::{browser::*, load_calendar, load_page, load_status, state};
use js_sys::{Array, Function, Promise};
use serde_json::{Value, json};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

#[derive(Clone)]
pub struct Lightbox {
    shot: bool,
    token: u64,
    requested: Value,
    detail: Value,
}

pub struct Confirmation(Function);

pub fn wire() {
    click("settingsButton", open_settings);
    for name in ["settingsClose", "cancelSettings", "drawerBackdrop"] {
        click(name, close_settings);
    }
    for (name, event_type) in [("settingsForm", "submit"), ("saveSettings", "click")] {
        on(&id(name), event_type, |event| {
            event.prevent_default();
            spawn(save_settings());
        });
    }
    click("settingsClearClips", || spawn(clear_clips()));
    click("settingsClearShots", || spawn(clear_shots()));

    click("lightboxClose", close_lightbox);
    on(&id("lightbox"), "click", |event| {
        if element_of(&event).is_some_and(|target| target.id() == "lightbox") {
            close_lightbox();
        }
    });
    click("lightboxCopy", || spawn(lightbox_call("copy")));
    click("lightboxReveal", || spawn(lightbox_call("reveal")));
    click("lightboxSave", || {
        spawn(async {
            if let Err(error) = save_lightbox().await {
                error_toast("", &error);
            }
        })
    });
    click("lightboxDelete", || {
        spawn(async {
            if let Some(lightbox) =
                state(|s| s.lightbox.clone()).filter(|item| !item.detail.is_null())
            {
                remove_item(lightbox.shot, lightbox.detail["item"].clone(), true).await;
            }
        })
    });

    click("modalConfirm", || finish_confirmation(true));
    click("modalCancel", || finish_confirmation(false));
    on(&id("modalBackdrop"), "click", |event| {
        if element_of(&event).is_some_and(|target| target.id() == "modalBackdrop") {
            finish_confirmation(false);
        }
    });
}

pub async fn load_settings() {
    match rpc("settings.get", Value::Null).await {
        Ok(value) => {
            state(|s| s.settings = value["settings"].clone());
            fill_settings();
        }
        Err(error) => error_toast("Не удалось загрузить настройки: ", &error),
    }
}

fn fill_settings() {
    let settings = state(|s| s.settings.clone());
    if settings.is_null() {
        return;
    }
    for (name, key) in [
        ("setCaptureEnabled", "captureEnabled"),
        ("setHonorExclusions", "honorClipboardExclusions"),
        ("setAutoImport", "autoImportScreenshots"),
        ("setRunInBackground", "runInBackground"),
    ] {
        input(name).set_checked(flag(&settings, key));
    }
    for (name, key, unit) in [
        ("setDedupe", "dedupeSeconds", 1.0),
        ("setMaxClips", "maxClips", 1.0),
        ("setMaxText", "maxTextBytes", 1024.0),
        ("setMaxShots", "maxScreenshotBytes", 1048576.0),
    ] {
        input(name).set_value(&(uint(&settings, key) as f64 / unit).round().to_string());
    }
    folders(&state(|s| s.status.clone()));
}

fn folders(status: &Value) {
    let list = id("folderList");
    list.set_inner_html("");
    let folders = array(status, "folders");
    if folders.is_empty() {
        append(
            &list,
            &el(
                "div",
                "folder-item",
                "Папки не найдены — импорт недоступен.",
            ),
        );
        return;
    }
    for folder in folders.iter().filter_map(Value::as_str).take(8) {
        let row = el("div", "folder-item", "");
        append(&row, &icon("folder"));
        let code = el("code", "", folder);
        set_attr(&code, "title", folder);
        append(&row, &code);
        append(&list, &row);
    }
    if folders.len() > 8 {
        append(
            &list,
            &el(
                "div",
                "folder-item",
                &format!("и ещё {}…", folders.len() - 8),
            ),
        );
    }
}

pub fn show_status(status: &Value) {
    set_text("aboutVersion", or(text(status, "version"), "—"));
    let background = if !flag(status, "runInBackground") {
        "Выключена"
    } else if flag(status, "backgroundRunning") {
        "Работает"
    } else {
        "Включена, ожидает запуска"
    };
    set_text("aboutBackground", background);
    let path = text(status, "dataDir");
    set_text("aboutDataDir", or(path, "—"));
    set_attr(&id("aboutDataDir"), "title", path);
    set_text("aboutStorage", &bytes(uint(status, "storageBytes")));
    if is_hidden("settingsDrawer") {
        fill_settings();
    } else {
        folders(status);
    }
}

fn open_settings() {
    fill_settings();
    hide("settingsDrawer", false);
    hide("drawerBackdrop", false);
    frame(|| {
        class(&id("settingsDrawer"), "is-open", true);
        set_attr(&id("settingsDrawer"), "aria-hidden", "false");
    });
    overlay();
    timeout(180, || focus("settingsClose"));
}

pub fn close_settings() {
    let drawer = id("settingsDrawer");
    class(&drawer, "is-open", false);
    set_attr(&drawer, "aria-hidden", "true");
    hide("drawerBackdrop", true);
    timeout(280, || {
        if is_hidden("drawerBackdrop") {
            hide("settingsDrawer", true);
            overlay();
        }
    });
}

async fn save_settings() {
    if state(|s| std::mem::replace(&mut s.saving, true)) {
        return;
    }
    let checked = |name| input(name).checked();
    let parsed = |name, fallback| {
        let value = input(name).value();
        let value = value.trim_start();
        let digits = value
            .char_indices()
            .take_while(|&(i, c)| c.is_ascii_digit() || i == 0 && matches!(c, '-' | '+'))
            .map(|(i, c)| i + c.len_utf8())
            .last()
            .unwrap_or_default();
        value[..digits]
            .parse::<i64>()
            .ok()
            .filter(|&n| n != 0)
            .unwrap_or(fallback)
    };
    let payload = json!({
        "captureEnabled":checked("setCaptureEnabled"),
        "honorClipboardExclusions":checked("setHonorExclusions"),
        "autoImportScreenshots":checked("setAutoImport"),
        "runInBackground":checked("setRunInBackground"),
        "dedupeSeconds":parsed("setDedupe",0),
        "maxClips":parsed("setMaxClips",0),
        "maxTextBytes":parsed("setMaxText",1024).saturating_mul(1024),
        "maxScreenshotBytes":parsed("setMaxShots",0).saturating_mul(1048576),
    });
    let button = id("saveSettings");
    let original = button.inner_html();
    disable(&button, true);
    button.set_inner_html("");
    append(&button, &icon("refresh"));
    append(&button, &el("span", "", "Сохранение…"));
    match rpc("settings.set", payload).await {
        Ok(response) => {
            state(|s| s.settings = response["settings"].clone());
            fill_settings();
            close_settings();
            toast("Настройки сохранены", "check", 3800);
            spawn(load_calendar(false));
            spawn(load_page(true, 0));
            load_status(true).await;
        }
        Err(error) => {
            error_toast("Не удалось сохранить: ", &error);
            spawn(load_status(true));
            load_settings().await;
        }
    }
    state(|s| s.saving = false);
    disable(&button, false);
    button.set_inner_html(&original);
    hydrate(&button);
}

fn resolve_confirmation(resolver: Function, ok: bool, checked: bool) {
    let data = Array::new();
    data.push(&JsValue::from_bool(ok));
    data.push(&JsValue::from_bool(checked));
    let _ = resolver.call1(&JsValue::UNDEFINED, &data);
}

async fn confirm(
    title: &str,
    copy: &str,
    action: &str,
    checkbox: Option<(&str, bool)>,
) -> Option<bool> {
    if let Some(Confirmation(resolver)) = state(|s| s.confirmation.take()) {
        resolve_confirmation(resolver, false, false);
    }
    set_text("modalTitle", title);
    set_text("modalText", copy);
    let button = id("modalConfirm");
    button.set_text_content(Some(action));
    button.set_class_name("danger-button");
    hide("modalCheckWrap", checkbox.is_none());
    let (label, checked) = checkbox.unwrap_or_default();
    input("modalCheck").set_checked(checked);
    set_text("modalCheckLabel", label);
    hide("modalBackdrop", false);
    overlay();
    timeout(30, || focus("modalConfirm"));
    let promise = Promise::new(&mut |resolve, _reject| {
        state(|s| s.confirmation = Some(Confirmation(resolve)))
    });
    let result = Array::from(&JsFuture::from(promise).await.ok()?);
    result
        .get(0)
        .as_bool()
        .unwrap_or(false)
        .then(|| result.get(1).as_bool().unwrap_or(false))
}

pub fn finish_confirmation(ok: bool) {
    let Some(Confirmation(resolver)) = state(|s| s.confirmation.take()) else {
        return;
    };
    let checked = input("modalCheck").checked();
    hide("modalBackdrop", true);
    overlay();
    resolve_confirmation(resolver, ok, checked);
}

async fn clear_clips() {
    let opt = confirm("Очистить историю буфера?", "Все записи ThodClip будут удалены. Системный буфер обмена и история Windows (Win+V) не изменятся.", "Очистить", Some(("Оставить закреплённые записи",true))).await;
    let Some(keep) = opt else {
        return;
    };
    match rpc("clips.clear", json!({"keepPinned":keep})).await {
        Ok(response) => {
            state(|s| {
                s.expanded.clear();
                s.full_texts.clear();
            });
            toast(
                &format!("Удалено: {}", number(uint(&response, "deleted"))),
                "check",
                3800,
            );
            spawn(load_calendar(false));
            spawn(load_status(true));
            load_page(true, 0).await;
        }
        Err(error) => error_toast("", &error),
    }
}

async fn clear_shots() {
    if confirm("Очистить архив скриншотов?", "Все архивные копии снимков будут удалены из папки данных ThodClip. Оригиналы в ваших папках останутся на месте.", "Очистить",None).await.is_none() { return; }
    match rpc("shots.clear", Value::Null).await {
        Ok(response) => {
            toast(
                &format!("Удалено снимков: {}", number(uint(&response, "deleted"))),
                "check",
                3800,
            );
            state(|s| {
                s.shots.items.clear();
                s.shots.total = 0;
            });
            load_page(false, 0).await;
        }
        Err(error) => error_toast("", &error),
    }
}

pub async fn remove_item(shot: bool, item: Value, lightbox: bool) {
    let kind = text(&item, "kind");
    let (title, copy) = if lightbox {
        (
            "Удалить изображение?",
            if shot {
                "Архивная копия будет удалена. Оригинал в исходной папке не затрагивается."
            } else if kind == "files" {
                "Запись будет удалена из истории ThodClip. Исходный файл не затрагивается."
            } else {
                "Изображение будет удалено из истории ThodClip."
            },
        )
    } else if shot {
        (
            "Удалить снимок?",
            "Архивная копия будет удалена из папки данных ThodClip. Оригинал, если он ещё существует, не затрагивается.",
        )
    } else {
        (
            "Удалить запись?",
            match kind {
                "text" => "Текст будет удалён из истории ThodClip без возможности восстановления.",
                "files" => "Запись со списком файлов будет удалена. Сами файлы останутся на диске.",
                _ => "Изображение будет удалено из истории и с диска внутри папки данных.",
            },
        )
    };
    if confirm(title, copy, "Удалить", None).await.is_none() {
        return;
    }
    match rpc(
        if shot { "shots.delete" } else { "clips.delete" },
        json!({"ids":[item["id"]]}),
    )
    .await
    {
        Ok(_) => {
            if lightbox {
                close_lightbox();
            }
            if !shot {
                let key = item["id"].as_i64().unwrap_or_default();
                state(|s| {
                    s.expanded.remove(&key);
                    s.full_texts.remove(&key);
                });
                spawn(load_calendar(false));
            }
            toast(
                if lightbox {
                    "Изображение удалено"
                } else if shot {
                    "Снимок удалён"
                } else {
                    "Запись удалена"
                },
                "check",
                3800,
            );
            load_page(!shot, 0).await;
        }
        Err(error) => error_toast("", &error),
    }
}

pub async fn open_lightbox(shot: bool, requested: Value) {
    let token = state(|s| {
        s.next_lightbox += 1;
        s.lightbox = Some(Lightbox {
            shot,
            token: s.next_lightbox,
            requested: requested.clone(),
            detail: Value::Null,
        });
        s.next_lightbox
    });
    hide("lightbox", false);
    hide("lightboxLoader", false);
    hide("lightboxNote", true);
    hide("lightboxReveal", false);
    disabled("lightboxSave", false);
    let image = id("lightboxImage");
    let _ = image.remove_attribute("src");
    set_text("lightboxTitle", "Изображение");
    set_text("lightboxMeta", "");
    overlay();
    let response = rpc(
        if shot { "shots.get" } else { "clips.get" },
        json!({"id":requested}),
    )
    .await;
    if !state(|s| s.lightbox.as_ref().is_some_and(|item| item.token == token)) {
        return;
    }
    match response {
        Ok(detail) => {
            let item = &detail["item"];
            let files = text(item, "kind") == "files";
            let name = text(item, "name");
            let alt = or(
                name,
                if files {
                    "Изображение из файла"
                } else if !shot {
                    "Изображение из буфера"
                } else {
                    "Снимок"
                },
            );
            set_attr(&image, "alt", alt);
            set_attr(&image, "src", text(&detail, "dataUrl"));
            let (width, height) = (number(uint(item, "width")), number(uint(item, "height")));
            let title = if shot {
                or(name, "Снимок").into()
            } else if files {
                or(first_filename(item), "Изображение").into()
            } else {
                format!("Изображение {width} × {height}")
            };
            set_text("lightboxTitle", &title);
            let mut bits = Vec::new();
            if !files {
                bits.push(format!("{width}×{height}"));
            }
            bits.push(bytes(uint(item, "bytes")));
            if !text(item, "source").is_empty() {
                bits.push(text(item, "source").into());
            }
            bits.push(short_date(text(item, "createdAt")));
            set_text("lightboxMeta", &bits.join(" · "));
            hide("lightboxLoader", true);
            let preview = flag(&detail, "preview");
            hide("lightboxNote", !preview);
            disabled("lightboxSave", preview);
            set_attr(
                &id("lightboxSave"),
                "title",
                if preview {
                    "Оригинал больше 5,5 МБ — откройте папку данных"
                } else {
                    "Сохранить как…"
                },
            );
            state(|s| {
                if let Some(item) = &mut s.lightbox {
                    item.detail = detail;
                }
            });
        }
        Err(error) => {
            error_toast("Не удалось открыть изображение: ", &error);
            close_lightbox();
        }
    }
}

pub fn close_lightbox() {
    hide("lightbox", true);
    let _ = id("lightboxImage").remove_attribute("src");
    state(|s| s.lightbox = None);
    overlay();
}

async fn lightbox_call(action: &str) {
    let Some(lightbox) = state(|s| s.lightbox.clone()).filter(|item| !item.detail.is_null()) else {
        return;
    };
    let method = format!("{}.{action}", if lightbox.shot { "shots" } else { "clips" });
    match rpc(&method, json!({"id":lightbox.requested})).await {
        Ok(_) if action == "copy" => toast("Скопировано в буфер", "check", 3800),
        Ok(_) => (),
        Err(error) => error_toast("", &error),
    }
}

fn first_filename(item: &Value) -> &str {
    basename(
        array(item, "files")
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
}

async fn save_lightbox() -> BrowserResult<()> {
    let Some(lightbox) =
        state(|s| s.lightbox.clone()).filter(|item| !text(&item.detail, "dataUrl").is_empty())
    else {
        return Ok(());
    };
    if flag(&lightbox.detail, "preview") {
        return Err("Оригинал больше 5,5 МБ — откройте его через кнопку «В папке».".into());
    }
    let item = &lightbox.detail["item"];
    let name = if lightbox.shot {
        shot_name(item)
    } else if text(item, "kind") == "files" && !first_filename(item).is_empty() {
        first_filename(item).into()
    } else {
        format!("clip-{}", item["id"])
    };
    export(&lightbox.detail, &name).await
}

fn shot_name(item: &Value) -> String {
    let name = text(item, "name");
    if name.is_empty() {
        format!("thodclip-{}", text(item, "id"))
    } else {
        name.to_owned()
    }
}

pub async fn save_shot(item: &Value) -> BrowserResult<()> {
    let detail = rpc("shots.get", json!({"id":item["id"]})).await?;
    export(&detail, &shot_name(item)).await
}

async fn export(detail: &Value, name: &str) -> BrowserResult<()> {
    let data = text(detail, "dataUrl");
    if data.is_empty() {
        return Err("Изображение недоступно.".into());
    }
    if flag(detail, "preview") {
        return Err(
            "Доступен только уменьшенный просмотр. Откройте оригинал в папке данных.".into(),
        );
    }
    let (header, base64) = data.split_once(',').ok_or("Изображение недоступно.")?;
    if base64.len() * 3 / 4 > 8 * 1024 * 1024 {
        return Err("Файл больше 8 МБ — сохраните его через папку данных.".into());
    }
    let mime = header
        .strip_prefix("data:")
        .and_then(|value| value.split_once(';'))
        .map_or(text(detail, "mime"), |p| p.0);
    let file_name = file_name(name, mime);
    let result = bridge("files.export", json!({"name":file_name,"base64":base64})).await?;
    if !flag(&result, "canceled") {
        toast(
            &format!("Сохранено: {}", or(text(&result, "filePath"), &file_name)),
            "save",
            3800,
        );
    }
    Ok(())
}

fn file_name(name: &str, mime: &str) -> String {
    let ext = match mime {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/webp" => "webp",
        "image/tiff" => "tiff",
        _ => "png",
    };
    let base = name
        .rsplit_once('.')
        .filter(|(_, ext)| {
            ["png", "jpg", "jpeg", "gif", "bmp", "webp", "tif", "tiff"]
                .contains(&ext.to_ascii_lowercase().as_str())
        })
        .map_or(name, |p| p.0);
    let base: String = base
        .chars()
        .map(|c| {
            if c <= '\u{1f}' || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let base: String = base
        .trim_end_matches(['.', ' '])
        .chars()
        .take(120)
        .collect();
    format!("{}.{ext}", or(&base, "thodclip"))
}
