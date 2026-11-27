use crate::{browser::*, load_calendar, load_page, overlays, state};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use web_sys::{Element, Event};

pub fn wire() {
    for (name, clips) in [("clipsList", true), ("shotsGrid", false)] {
        on(&id(name), "click", move |event| action_click(event, clips));
    }
}

fn action_click(event: Event, clips: bool) {
    let Some(button) = element_of(&event).and_then(|el| el.closest("[data-action]").ok().flatten())
    else {
        return;
    };
    let Some(card) = button.closest("article[data-id]").ok().flatten() else {
        return;
    };
    let key = attr(&card, "data-id");
    let item = state(|s| {
        s.page(clips)
            .items
            .iter()
            .find(|item| item_id(item) == key)
            .cloned()
    });
    let Some(item) = item else {
        return;
    };
    event.stop_propagation();
    let action = attr(&button, "data-action");
    spawn(async move {
        if action == "open" {
            overlays::open_lightbox(!clips, item["id"].clone()).await;
            return;
        }
        if action == "delete" {
            overlays::remove_item(!clips, item, false).await;
            return;
        }
        disable(&button, true);
        let result = if clips {
            match action.as_str() {
                "copy" | "paths" => copy_clip(&item, &card, action == "paths").await,
                "pin" => pin_clip(&item).await,
                "expand" => expand_clip(&item, &card, &button).await,
                _ => Ok(()),
            }
        } else {
            let id = item["id"].clone();
            match action.as_str() {
                "copy" => rpc("shots.copy", json!({"id":id}))
                    .await
                    .map(|_| toast("Изображение скопировано", "check", 3800)),
                "save" => overlays::save_shot(&item).await,
                "reveal" => rpc("shots.reveal", json!({"id":id})).await.map(|_| ()),
                _ => Ok(()),
            }
        };
        disable(&button, false);
        if let Err(error) = result {
            error_toast("", &error);
        }
    });
}

fn item_id(item: &Value) -> String {
    item["id"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| item["id"].to_string())
}

fn render_summary(prefix: &str, count: usize, total: u64) {
    hide(
        if prefix == "clips" {
            "loadMoreClips"
        } else {
            "loadMoreShots"
        },
        count as u64 >= total,
    );
    let summary = if total == 0 {
        String::new()
    } else {
        format!("Показано {} из {}", number(count as u64), number(total))
    };
    set_text(&format!("{prefix}TotalLabel"), &summary);
    set_text(
        if prefix == "clips" {
            "clipTabCount"
        } else {
            "shotTabCount"
        },
        &number(total),
    );
}

fn empty_text(name: &str, heading: &str, description: &str) {
    let node = id(name);
    for (selector, copy) in [("h3", heading), ("p", description)] {
        if let Ok(Some(target)) = node.query_selector(selector) {
            target.set_text_content(Some(copy));
        }
    }
}

pub fn render_clips() {
    state(|s| {
        let list = id("clipsList");
        list.set_inner_html("");
        let mut groups: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
        let mut pinned = Vec::new();
        for item in &s.clips.items {
            if flag(item, "pinned") {
                pinned.push(item);
            } else {
                groups
                    .entry(local_date(text(item, "createdAt")))
                    .or_default()
                    .push(item);
            }
        }
        let clip_group = |title: &str, items: &[&Value]| {
            let group = el("section", "day-group", "");
            let heading = el("div", "day-heading", "");
            let count = items.len() as u64;
            append(&heading, &el("strong", "", title));
            append(
                &heading,
                &el("span", "", &format!("{} {}", number(count), records(count))),
            );
            append(&group, &heading);
            for &item in items {
                let key = item["id"].as_i64().unwrap_or_default();
                append(
                    &group,
                    &clip_card(
                        item,
                        s.expanded.contains(&key),
                        s.full_texts.get(&key).map(String::as_str),
                    ),
                );
            }
            append(&list, &group);
        };
        if !pinned.is_empty() {
            clip_group("Закреплённые", &pinned);
        }
        for (day, items) in groups.iter().rev() {
            clip_group(&day_title(day), items);
        }
        let count = s.clips.items.len();
        hide("clipsEmpty", count != 0);
        if count == 0 {
            let filtered = !s.query.is_empty()
                || matches!(s.filter.as_str(), "text" | "files" | "image" | "pinned")
                || s.date.is_some();
            if filtered {
                empty_text(
                    "clipsEmpty",
                    "Ничего не найдено",
                    "Попробуйте изменить запрос, фильтр или выбранный день.",
                );
            } else {
                empty_text(
                    "clipsEmpty",
                    "Пока пусто",
                    "Скопируйте что-нибудь — текст, файлы или картинку. ThodClip сохранит запись сюда и не потеряет её после перезапуска.",
                );
            }
        }
        render_summary("clips", count, s.clips.total);
    });
}

fn button(class: &str, action: &str, title: &str) -> Element {
    let button = el("button", class, "");
    set_attr(&button, "type", "button");
    set_attr(&button, "data-action", action);
    if !title.is_empty() {
        set_attr(&button, "title", title);
    }
    button
}

fn action(action: &str, symbol: &str, label: &str, title: &str, primary: bool) -> Element {
    let button = button(
        if primary {
            "action-button primary"
        } else {
            "action-button"
        },
        action,
        title,
    );
    append(&button, &icon(symbol));
    if !label.is_empty() {
        append(&button, &el("span", "", label));
    }
    button
}

fn image(src: &str, alt: &str) -> Element {
    let img = el("img", "", "");
    set_attr(&img, "src", src);
    set_attr(&img, "alt", alt);
    set_attr(&img, "loading", "lazy");
    img
}

fn delete_button(class: &str, label: &str) -> Element {
    let remove = button(class, "delete", label);
    set_attr(&remove, "aria-label", label);
    append(&remove, &icon("close"));
    remove
}

fn clip_card(item: &Value, expanded: bool, cached: Option<&str>) -> Element {
    let kind = text(item, "kind");
    let pinned = flag(item, "pinned");
    let card = el(
        "article",
        if pinned {
            "clip-card is-pinned"
        } else {
            "clip-card"
        },
        "",
    );
    set_attr(&card, "data-kind", kind);
    set_attr(&card, "data-id", &item_id(item));

    let (symbol, type_name) = match kind {
        "image" => ("image", "Изображение"),
        "files" => ("file", "Файлы"),
        _ => ("text", "Текст"),
    };
    let type_label = el("div", "clip-type", "");
    append(&type_label, &icon(symbol));
    append(&card, &type_label);
    let main = el("div", "clip-main", "");
    let meta = el("div", "clip-meta", "");
    let source = or(text(item, "source"), type_name);
    let source_label = el("span", "source", source);
    set_attr(&source_label, "title", source);
    append(&meta, &source_label);
    append(&meta, &el("time", "", &date_time(text(item, "createdAt"))));
    let repeats = uint(item, "repeatCount");
    if repeats > 1 {
        append(
            &meta,
            &el("span", "badge repeat", &format!("×{}", number(repeats))),
        );
    }
    if pinned {
        append(&meta, &el("span", "badge pin", "закреплено"));
    }
    if flag(item, "truncated") {
        append(&meta, &el("span", "badge", "обрезано"));
    }
    append(&meta, &delete_button("clip-delete", "Удалить запись"));
    append(&main, &meta);

    let files = array(item, "files");
    if kind == "image" {
        let (width, height) = (uint(item, "width"), uint(item, "height"));
        let filename = files
            .iter()
            .filter_map(Value::as_str)
            .map(basename)
            .find(|name| !name.is_empty() && !temporary(name));
        let caption = format!(
            "{}{}{} × {} · {}",
            filename.unwrap_or("Изображение"),
            if filename.is_some() { " · " } else { " " },
            number(width),
            number(height),
            bytes(uint(item, "bytes"))
        );
        let label = el("p", "clip-text", &caption);
        if filename.is_some() {
            set_attr(
                &label,
                "title",
                files.first().and_then(Value::as_str).unwrap_or_default(),
            );
        }
        append(&main, &label);
        clip_thumb(
            &main,
            text(item, "thumb"),
            "clip-thumb",
            &format!("Изображение {width} на {height}"),
        );
    } else if kind == "files" {
        clip_thumb(
            &main,
            text(item, "thumb"),
            "clip-thumb clip-thumb-mini",
            "Миниатюра изображения",
        );
        let list = el("ul", "clip-files", "");
        for file in files.iter().filter_map(Value::as_str).take(4) {
            let row = el("li", "file-line", "");
            append(&row, &icon("file"));
            let label = el("span", "", or(basename(file), file));
            set_attr(&label, "title", file);
            append(&row, &label);
            append(&list, &row);
        }
        if files.len() > 4 {
            append(
                &list,
                &el(
                    "li",
                    "file-more",
                    &format!("и ещё {}", number((files.len() - 4) as u64)),
                ),
            );
        }
        append(&main, &list);
    } else {
        let plain = text(item, "text");
        let clipped = flag(item, "textClipped");
        let visible = if expanded {
            cached.unwrap_or(plain)
        } else {
            plain
        };
        append(
            &main,
            &el(
                "p",
                if expanded {
                    "clip-text is-expanded"
                } else {
                    "clip-text"
                },
                visible,
            ),
        );
        if clipped
            || plain.encode_utf16().count() > 700
            || plain.bytes().filter(|&b| b == b'\n').count() > 7
        {
            let expand = button("clip-expand", "expand", "");
            expand_label(&expand, expanded, visible.encode_utf16().count(), clipped);
            append(&main, &expand);
        }
    }

    append(&card, &main);
    let actions = el("div", "clip-actions", "");
    append(
        &actions,
        &action("copy", "copy", "Копировать", "Скопировать", true),
    );
    if kind == "files" {
        append(&actions, &action("paths", "file", "Пути", "Пути", false));
    }
    let (symbol, label, title) = if pinned {
        ("pin-off", "Снять", "Снять с закреплённых")
    } else {
        ("pin", "Закрепить", "Закрепить")
    };
    append(&actions, &action("pin", symbol, label, title, false));
    append(&card, &actions);
    card
}

fn temporary(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let stem = lower
        .rsplit_once('.')
        .filter(|(_, ext)| {
            !ext.is_empty() && ext.len() <= 5 && ext.bytes().all(|c| c.is_ascii_alphanumeric())
        })
        .map_or(lower.as_str(), |(stem, _)| stem);
    let uuid = stem.trim_matches(['{', '}']).split('-').collect::<Vec<_>>();
    let hex = |s: &str| s.bytes().all(|c| c.is_ascii_hexdigit());
    (uuid.len() == 5
        && uuid
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(s, len)| s.len() == len && hex(s)))
        || (stem.len() >= 16 && stem != lower && hex(stem))
        || ["tmp-", "tmp_", "temp-", "temp_", "~-", "~_"]
            .iter()
            .any(|prefix| lower.starts_with(prefix))
}

fn clip_thumb(main: &Element, url: &str, class: &str, alt: &str) {
    if url.is_empty() {
        return;
    }
    let button = button(class, "open", "Открыть изображение");
    append(&button, &image(url, alt));
    append(main, &button);
}

fn expand_label(button: &Element, expanded: bool, count: usize, clipped: bool) {
    button.set_inner_html("");
    append(
        button,
        &icon(if expanded {
            "chevron-up"
        } else {
            "chevron-down"
        }),
    );
    append(
        button,
        &el(
            "span",
            "",
            if expanded {
                "Свернуть"
            } else {
                "Раскрыть"
            },
        ),
    );
    let count = if clipped && !expanded {
        "полный текст".into()
    } else {
        format!("{} симв.", number(count as u64))
    };
    append(button, &el("span", "chars", &count));
}

async fn expand_clip(item: &Value, card: &Element, button: &Element) -> BrowserResult<()> {
    let key = item["id"].as_i64().unwrap_or_default();
    let (collapse, cached) = state(|s| (s.expanded.remove(&key), s.full_texts.get(&key).cloned()));
    let plain = text(item, "text");
    let clipped = flag(item, "textClipped");
    let Some(node) = card.query_selector(".clip-text").ok().flatten() else {
        return Ok(());
    };
    if collapse {
        node.set_text_content(Some(cached.as_deref().unwrap_or(plain)));
        class(&node, "is-expanded", false);
        expand_label(button, false, plain.encode_utf16().count(), clipped);
        return Ok(());
    }
    let full = if let Some(full) = cached {
        full
    } else if !clipped {
        plain.to_owned()
    } else {
        button.set_inner_html("");
        append(button, &icon("refresh"));
        append(button, &el("span", "", "Загрузка…"));
        match rpc("clips.get", json!({"id":key})).await {
            Ok(detail) => {
                let full = text(&detail["item"], "text").to_owned();
                state(|s| {
                    s.full_texts.insert(key, full.clone());
                });
                full
            }
            Err(error) => {
                expand_label(button, false, plain.encode_utf16().count(), clipped);
                return Err(format!("Не удалось раскрыть текст: {error}"));
            }
        }
    };
    state(|s| {
        s.expanded.insert(key);
    });
    let full = or(&full, plain);
    node.set_text_content(Some(full));
    class(&node, "is-expanded", true);
    expand_label(button, true, full.encode_utf16().count(), false);
    Ok(())
}

async fn copy_clip(item: &Value, card: &Element, paths: bool) -> BrowserResult<()> {
    let mode = if paths { "paths" } else { "default" };
    rpc("clips.copy", json!({"id":item["id"],"mode":mode})).await?;
    toast(
        if paths {
            "Пути скопированы"
        } else if text(item, "kind") == "files" {
            "Файлы скопированы"
        } else {
            "Скопировано в буфер"
        },
        "check",
        3800,
    );
    if !paths {
        class(card, "is-copied", true);
        let card = card.clone();
        timeout(900, move || class(&card, "is-copied", false));
    }
    Ok(())
}

async fn pin_clip(item: &Value) -> BrowserResult<()> {
    let key = item["id"].as_i64().unwrap_or_default();
    rpc("clips.pin", json!({"id":key,"pinned":!flag(item,"pinned")})).await?;
    state(|s| {
        s.full_texts.remove(&key);
        s.expanded.remove(&key);
    });
    spawn(load_calendar(false));
    load_page(true, 0).await;
    Ok(())
}

pub fn render_shots() {
    state(|s| {
        let grid = id("shotsGrid");
        grid.set_inner_html("");
        for item in &s.shots.items {
            append(&grid, &shot_card(item));
        }
        let count = s.shots.items.len();
        hide("shotsEmpty", count != 0);
        let filtered = !s.query.is_empty();
        hide("importShotsEmpty", count == 0 && filtered);
        if count == 0 {
            if filtered {
                empty_text(
                    "shotsEmpty",
                    "Ничего не найдено",
                    "Попробуйте изменить поисковый запрос.",
                );
            } else {
                empty_text(
                    "shotsEmpty",
                    "Снимков пока нет",
                    "Сделайте скриншот через Snipping Tool или сохраните его в папку «Изображения». ThodClip найдёт и заархивирует файл.",
                );
            }
        }
        render_summary("shots", count, s.shots.total);
    });
}

fn shot_card(item: &Value) -> Element {
    let card = el("article", "shot-card", "");
    set_attr(&card, "data-id", &item_id(item));
    let thumb = button("shot-thumb", "open", "Открыть снимок");
    let name = text(item, "name");
    if text(item, "thumb").is_empty() {
        append(&thumb, &icon("image"));
    } else {
        append(&thumb, &image(text(item, "thumb"), or(name, "Скриншот")));
    }
    let open = el("span", "shot-open", "");
    append(&open, &icon("external"));
    append(&thumb, &open);
    append(&card, &thumb);

    let info = el("div", "shot-info", "");
    append(&info, &el("p", "shot-name", or(name, "Снимок")));
    let metadata = format!(
        "{}×{} · {} · {}",
        number(uint(item, "width")),
        number(uint(item, "height")),
        bytes(uint(item, "bytes")),
        short_date(text(item, "createdAt"))
    );
    append(&info, &el("p", "shot-meta", &metadata));
    append(&card, &info);

    let actions = el("div", "shot-actions", "");
    for (name, symbol, title) in [
        ("copy", "copy", "Копировать изображение"),
        ("save", "save", "Сохранить как…"),
        ("reveal", "folder", "Показать в папке"),
    ] {
        append(&actions, &action(name, symbol, "", title, false));
    }
    append(&card, &actions);
    append(&card, &delete_button("shot-delete", "Удалить снимок"));
    card
}
