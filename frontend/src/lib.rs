#![cfg(target_arch = "wasm32")]

mod browser;
mod cards;
mod overlays;

use browser::*;
use js_sys::Date;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
};
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
use web_sys::{Event, KeyboardEvent};

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::new());
}

const QUERY_KEY: &str = "thodclip.search";

#[derive(Default)]
struct Page {
    items: Vec<Value>,
    total: u64,
    loading: bool,
    pending: bool,
}

struct State {
    tab: String,
    query: String,
    filter: String,
    date: Option<String>,
    month: (u32, u32),
    calendar: Value,
    calendar_loading: bool,
    calendar_pending: bool,
    clips: Page,
    shots: Page,
    status: Value,
    settings: Value,
    saving: bool,
    expanded: HashSet<i64>,
    full_texts: HashMap<i64, String>,
    lightbox: Option<overlays::Lightbox>,
    confirmation: Option<overlays::Confirmation>,
    query_timer: Option<i32>,
    calendar_timer: Option<i32>,
    clips_timer: Option<i32>,
    import_timer: Option<i32>,
    import_ticks: u8,
    last_import_error: String,
    next_lightbox: u64,
}

impl State {
    fn new() -> Self {
        Self {
            tab: "clips".into(),
            query: String::new(),
            filter: String::new(),
            date: None,
            month: current_month(),
            calendar: Value::Null,
            calendar_loading: false,
            calendar_pending: false,
            clips: Page::default(),
            shots: Page::default(),
            status: Value::Null,
            settings: Value::Null,
            saving: false,
            expanded: HashSet::new(),
            full_texts: HashMap::new(),
            lightbox: None,
            confirmation: None,
            query_timer: None,
            calendar_timer: None,
            clips_timer: None,
            import_timer: None,
            import_ticks: 0,
            last_import_error: String::new(),
            next_lightbox: 0,
        }
    }

    fn page(&mut self, clips: bool) -> &mut Page {
        if clips {
            &mut self.clips
        } else {
            &mut self.shots
        }
    }
}

fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|state| f(&mut state.borrow_mut()))
}

#[wasm_bindgen(start)]
pub fn start() {
    theme();
    hydrate(&document().document_element().unwrap());
    wire();
    if js_sys::Reflect::get(&window(), &JsValue::from_str("thodHub"))
        .is_ok_and(|value| value.is_undefined())
    {
        toast(
            "Нет связи с ThodHUB. Откройте ThodClip через каталог.",
            "alert",
            12000,
        );
        return;
    }
    if let Some(saved) = window()
        .session_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(QUERY_KEY).ok().flatten())
        .filter(|s| !s.is_empty())
    {
        input("searchInput").set_value(&saved);
        hide("searchClear", false);
        state(|s| s.query = saved);
    }
    spawn(async {
        overlays::load_settings().await;
        load_status(true).await;
        load_calendar(true).await;
        load_page(true, 0).await;
        interval(5000, || {
            spawn(async {
                if !document().hidden() {
                    load_status(false).await;
                }
            })
        });
    });
}

fn theme() {
    let query = window().location().search().unwrap_or_default();
    let params = web_sys::UrlSearchParams::new_with_str(&query).ok();
    let theme = match params.and_then(|p| p.get("theme")).as_deref() {
        Some("light") => "light",
        Some("dark") => "dark",
        _ if window()
            .match_media("(prefers-color-scheme: dark)")
            .ok()
            .flatten()
            .is_some_and(|query| query.matches()) =>
        {
            "dark"
        }
        _ => "light",
    };
    set_attr(&document().document_element().unwrap(), "data-theme", theme);
}

fn reload(offset: usize) {
    let clips = state(|s| s.tab == "clips");
    spawn(load_page(clips, offset));
}

fn set_query(clear: bool) {
    if clear {
        input("searchInput").set_value("");
    }
    let text = input("searchInput").value();
    hide("searchClear", text.is_empty());
    let old = state(|s| s.query_timer.take());
    if let Some(id) = old {
        window().clear_timeout_with_handle(id);
    }
    let apply = move || {
        let query = input("searchInput").value().trim().to_owned();
        if let Ok(Some(storage)) = window().session_storage() {
            if clear {
                let _ = storage.remove_item(QUERY_KEY);
            } else {
                let _ = storage.set_item(QUERY_KEY, &query);
            }
        }
        state(|s| {
            s.query = query;
            s.query_timer = None;
        });
        reload(0);
    };
    if clear {
        apply();
        focus("searchInput");
    } else {
        state(|s| s.query_timer = Some(timeout(280, apply)));
    }
}

fn wire() {
    for tab in select(".tab") {
        let value = attr(&tab, "data-tab");
        on(&tab.clone(), "click", move |_| {
            if state(|s| s.tab == value) {
                return;
            }
            state(|s| s.tab.clone_from(&value));
            for node in select(".tab") {
                class(&node, "is-active", attr(&node, "data-tab") == value);
            }
            let clips = value == "clips";
            class(&id("clipsView"), "is-active", clips);
            class(&id("shotsView"), "is-active", !clips);
            input("searchInput").set_placeholder(if clips {
                "Поиск по истории"
            } else {
                "Поиск по снимкам"
            });
            reload(0);
        });
    }
    for chip in select(".chip") {
        let value = attr(&chip, "data-filter");
        on(&chip, "click", move |_| {
            for node in select(".chip") {
                class(&node, "is-active", attr(&node, "data-filter") == value);
            }
            state(|s| s.filter.clone_from(&value));
            spawn(load_page(true, 0));
        });
    }
    on(&id("searchInput"), "input", |_| set_query(false));
    click("searchClear", || set_query(true));
    click("capturePill", || spawn(toggle_capture()));
    click("refreshButton", || {
        spawn(async {
            class(&id("refreshButton"), "is-spinning", true);
            load_status(true).await;
            load_calendar(false).await;
            let clips = state(|s| s.tab == "clips");
            load_page(clips, 0).await;
            toast("Обновлено", "check", 3800);
            class(&id("refreshButton"), "is-spinning", false);
        })
    });
    for (button, delta) in [
        ("calendarPrev", -1),
        ("calendarNext", 1),
        ("calendarToday", 0),
    ] {
        click(button, move || {
            state(|s| {
                s.month = if delta == 0 {
                    current_month()
                } else {
                    shift_month(s.month, delta)
                }
            });
            spawn(load_calendar(false));
        });
    }
    click("dayBannerClear", || select_day(None));
    click("newItemsRefresh", || {
        hide("newItems", true);
        spawn(load_page(true, 0));
        spawn(load_calendar(false));
        spawn(load_status(true));
    });
    click("loadMoreClips", || {
        spawn(load_page(true, state(|s| s.clips.items.len())))
    });
    click("loadMoreShots", || {
        spawn(load_page(false, state(|s| s.shots.items.len())))
    });
    for button in ["openDataFolder", "settingsOpenFolder", "openShotsFolder"] {
        click(button, || {
            spawn(async {
                if let Err(error) = rpc("storage.reveal", Value::Null).await {
                    error_toast("", &error);
                }
            })
        });
    }
    for button in [
        "importShots",
        "importShotsEmpty",
        "settingsImport",
        "importFromSettings",
    ] {
        click(button, || {
            spawn(async {
                match rpc("shots.import", Value::Null).await {
                    Ok(value) => {
                        toast(
                            or(text(&value, "message"), "Импорт запущен"),
                            "download",
                            3800,
                        );
                        watch_import();
                    }
                    Err(error) => error_toast("", &error),
                }
            })
        });
    }
    on(&id("calendarGrid"), "click", calendar_click);
    cards::wire();
    overlays::wire();
    on(&document(), "visibilitychange", |_| {
        if !document().hidden() {
            spawn(load_status(false));
        }
    });
    on(&document(), "keydown", |event| {
        let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        if key.key() == "Enter" && !is_hidden("modalBackdrop") {
            if element_of(&event)
                .is_some_and(|el| el.tag_name() == "BUTTON" && el.id() != "modalConfirm")
            {
                return;
            }
            event.prevent_default();
            overlays::finish_confirmation(true);
        } else if key.key() == "Escape" {
            if !is_hidden("modalBackdrop") {
                overlays::finish_confirmation(false);
            } else if !is_hidden("lightbox") {
                overlays::close_lightbox();
            } else if !is_hidden("settingsDrawer") {
                overlays::close_settings();
            } else {
                return;
            }
            event.prevent_default();
        }
    });
}

async fn load_page(clips: bool, mut offset: usize) {
    loop {
        let params = state(|s| {
            let page = s.page(clips);
            if page.loading {
                if offset == 0 {
                    page.pending = true;
                }
                return None;
            }
            page.loading = true;
            if clips {
                let kind =
                    matches!(s.filter.as_str(), "text" | "files" | "image").then_some(&s.filter);
                Some(
                    json!({"query":s.query,"kind":kind,"pinned":s.filter=="pinned","date":s.date,"offset":offset,"limit":50}),
                )
            } else {
                Some(json!({"query":s.query,"offset":offset,"limit":36}))
            }
        });
        let Some(params) = params else {
            return;
        };
        class(&id("refreshButton"), "is-spinning", true);
        let (method, failure) = if clips {
            ("clips.list", "Не удалось загрузить историю: ")
        } else {
            ("shots.list", "Не удалось загрузить снимки: ")
        };
        match rpc(method, params).await {
            Ok(page) => {
                state(|s| {
                    let dest = s.page(clips);
                    if offset == 0 {
                        dest.items.clear();
                    }
                    let existing = dest
                        .items
                        .iter()
                        .map(|entry| entry["id"].clone())
                        .collect::<Vec<_>>();
                    dest.items.extend(
                        array(&page, "items")
                            .iter()
                            .filter(|entry| !existing.contains(&entry["id"]))
                            .cloned(),
                    );
                    dest.total = uint(&page, "total");
                });
                if clips {
                    hide("newItems", true);
                    cards::render_clips();
                    update_day_banner();
                } else {
                    cards::render_shots();
                }
            }
            Err(error) => error_toast(failure, &error),
        }
        class(&id("refreshButton"), "is-spinning", false);
        let pending = state(|s| {
            let page = s.page(clips);
            page.loading = false;
            std::mem::take(&mut page.pending)
        });
        if !pending {
            break;
        }
        offset = 0;
    }
}

async fn load_status(announce_error: bool) {
    match rpc("status", Value::Null).await {
        Ok(status) => apply_status(status),
        Err(error) if announce_error => error_toast("Не удалось получить состояние: ", &error),
        Err(_) => {}
    }
}

fn apply_status(status: Value) {
    let previous = state(|s| std::mem::replace(&mut s.status, status.clone()));
    update_capture(&status);
    for (id, key) in [
        ("clipTabCount", "clipCount"),
        ("shotTabCount", "shotCount"),
        ("storageClips", "clipCount"),
        ("storageShots", "shotCount"),
    ] {
        set_text(id, &number(uint(&status, key)));
    }
    let size = bytes(uint(&status, "storageBytes"));
    set_text("storageSize", &size);
    overlays::show_status(&status);
    if flag(&status["import"], "running") {
        watch_import();
    }
    let error = text(&status["import"], "error");
    if !error.is_empty()
        && !flag(&status["import"], "running")
        && state(|s| {
            if s.last_import_error == error {
                return false;
            }
            s.last_import_error = error.to_owned();
            true
        })
    {
        toast(&format!("Импорт скриншотов: {error}"), "alert", 6500);
    }
    let clips = uint(&status, "clipCount");
    let old_clips = uint(&previous, "clipCount");
    let on_clips = state(|s| s.tab == "clips");
    if !previous.is_null() && clips != old_clips {
        schedule_calendar();
        if clips > old_clips && on_clips && no_overlay() {
            if window().scroll_y().unwrap_or_default() < 140.0 {
                schedule_clips();
            } else {
                hide("newItems", false);
            }
        }
    }
    if !previous.is_null()
        && uint(&status, "shotCount") > uint(&previous, "shotCount")
        && !on_clips
        && is_hidden("lightbox")
    {
        spawn(load_page(false, 0));
    }
}

fn update_capture(status: &Value) {
    let enabled = flag(status, "captureEnabled");
    let active = status["captureActive"].as_bool().unwrap_or(true);
    class(&id("capturePill"), "is-off", !enabled || !active);
    set_text(
        "captureLabel",
        if !enabled {
            "Запись выключена"
        } else if active {
            "Запись включена"
        } else {
            "Запись не активна"
        },
    );
    let owner = match text(status, "captureOwner") {
        "background" => "фоновая запись",
        "hub" => "ThodHUB",
        _ => "",
    };
    let title = if !enabled {
        "Включить запись буфера обмена".into()
    } else if owner.is_empty() {
        "Выключить запись буфера обмена".into()
    } else {
        format!("Выключить запись буфера обмена · сейчас: {owner}")
    };
    set_attr(&id("capturePill"), "title", &title);
}

async fn toggle_capture() {
    let next = state(|s| !s.status["captureEnabled"].as_bool().unwrap_or(true));
    disabled("capturePill", true);
    match rpc("capture.set", json!({"enabled":next})).await {
        Ok(_) => {
            let status = state(|s| {
                s.status["captureEnabled"] = json!(next);
                s.status.clone()
            });
            update_capture(&status);
            if !is_hidden("settingsDrawer") {
                input("setCaptureEnabled").set_checked(next);
            }
            toast(
                if next {
                    "Запись буфера включена"
                } else {
                    "Запись буфера выключена"
                },
                if next { "check" } else { "info" },
                3800,
            );
        }
        Err(error) => error_toast("", &error),
    }
    disabled("capturePill", false);
}

fn no_overlay() -> bool {
    ["lightbox", "modalBackdrop", "settingsDrawer"]
        .into_iter()
        .all(is_hidden)
}

fn schedule_clips() {
    if let Some(id) = state(|s| s.clips_timer.take()) {
        window().clear_timeout_with_handle(id);
    }
    let id = timeout(320, || {
        state(|s| s.clips_timer = None);
        if state(|s| s.tab == "clips") && no_overlay() {
            spawn(load_page(true, 0));
        }
    });
    state(|s| s.clips_timer = Some(id));
}

fn schedule_calendar() {
    if state(|s| s.calendar_timer.is_some()) {
        return;
    }
    let id = timeout(500, || {
        state(|s| s.calendar_timer = None);
        spawn(load_calendar(false));
    });
    state(|s| s.calendar_timer = Some(id));
}

fn watch_import() {
    if state(|s| s.import_timer.is_some()) {
        return;
    }
    let timer = interval(650, || {
        spawn(async {
            let ticks = state(|s| {
                s.import_ticks += 1;
                s.import_ticks
            });
            match rpc("status", Value::Null).await {
                Ok(status) => {
                    apply_status(status.clone());
                    let import = &status["import"];
                    if !flag(import, "running") || ticks >= 90 {
                        stop_import_timer();
                        let (added, failed) = (uint(import, "imported"), uint(import, "failed"));
                        if added > 0 || failed > 0 {
                            let mut message =
                                format!("Импорт скриншотов: добавлено {}", number(added));
                            if failed > 0 {
                                message.push_str(&format!(", ошибок {}", number(failed)));
                            }
                            toast(&message, "image", 3800);
                        }
                        if state(|s| s.tab == "shots") {
                            load_page(false, 0).await;
                        }
                    }
                }
                Err(_) => stop_import_timer(),
            }
        })
    });
    state(|s| {
        s.import_timer = Some(timer);
        s.import_ticks = 0;
    });
}

fn stop_import_timer() {
    if let Some(timer) = state(|s| s.import_timer.take()) {
        window().clear_interval_with_handle(timer);
    }
}

fn update_day_banner() {
    let (date, count) = state(|s| (s.date.clone(), s.clips.total));
    hide("dayBanner", date.is_none());
    if let Some(date) = date {
        set_text(
            "dayBannerText",
            &format!("{} · {} {}", day_long(&date), number(count), records(count)),
        );
    }
}

fn select_day(date: Option<String>) {
    let selected = state(|s| {
        s.date = if s.date == date { None } else { date };
        if let Some(day) = &s.date {
            s.month = month_from_key(day);
        }
        s.date.is_some()
    });
    if selected {
        render_calendar();
    }
    update_day_banner();
    spawn(async {
        load_page(true, 0).await;
        if state(|s| s.date.is_some())
            && let Ok(Some(target)) = document().query_selector(".content")
        {
            let options = web_sys::ScrollIntoViewOptions::new();
            options.set_behavior(web_sys::ScrollBehavior::Smooth);
            options.set_block(web_sys::ScrollLogicalPosition::Start);
            target.scroll_into_view_with_scroll_into_view_options(&options);
        }
    });
}

async fn load_calendar(mut auto_jump: bool) {
    loop {
        let month = state(|s| {
            if s.calendar_loading {
                s.calendar_pending = true;
                return None;
            }
            s.calendar_loading = true;
            Some(s.month)
        });
        let Some((year, month)) = month else {
            return;
        };
        let response = rpc(
            "clips.calendar",
            json!({"month":format!("{year:04}-{month:02}")}),
        )
        .await;
        match response {
            Ok(data) => {
                let last = text(&data, "lastDate");
                if auto_jump
                    && uint(&data, "total") == 0
                    && !last.is_empty()
                    && month_from_key(last) != (year, month)
                {
                    state(|s| {
                        s.month = month_from_key(last);
                        s.calendar_loading = false;
                    });
                    auto_jump = false;
                    continue;
                }
                state(|s| s.calendar = data);
                render_calendar();
            }
            Err(error) => error_toast("Не удалось загрузить календарь: ", &error),
        }
        let pending = state(|s| {
            s.calendar_loading = false;
            std::mem::take(&mut s.calendar_pending)
        });
        if !pending {
            break;
        }
        auto_jump = false;
    }
}

fn render_calendar() {
    let ((year, month), date, calendar) = state(|s| (s.month, s.date.clone(), s.calendar.clone()));
    let counts = array(&calendar, "days")
        .iter()
        .map(|day| (text(day, "date"), uint(day, "count")))
        .collect::<BTreeMap<_, _>>();
    let max = counts.values().copied().max().unwrap_or(0);
    let offset = (Date::new_with_year_month_day(year, month as i32 - 1, 1).get_day() + 6) % 7;
    let days = Date::new_with_year_month_day(year, month as i32, 0).get_date();
    let today = local_key(&Date::new_0());
    set_text(
        "calendarTitle",
        &format!("{} {year}", MONTHS_NOM[month as usize - 1]),
    );
    let grid = id("calendarGrid");
    grid.set_inner_html("");
    for _ in 0..offset {
        append(&grid, &el("span", "day is-empty", ""));
    }
    for day in 1..=days {
        let key = format!("{year:04}-{month:02}-{day:02}");
        let count = counts.get(key.as_str()).copied().unwrap_or(0);
        let button = el("button", "day", "");
        set_attr(&button, "type", "button");
        set_attr(&button, "data-date", &key);
        if count > 0 {
            let ratio = count as f64 / max as f64;
            let heat = if max <= 1 {
                1
            } else if ratio > 0.66 {
                4
            } else if ratio > 0.4 {
                3
            } else if ratio > 0.15 {
                2
            } else {
                1
            };
            class(&button, &format!("heat-{heat}"), true);
        }
        class(&button, "is-today", key == today);
        class(&button, "is-selected", date.as_ref() == Some(&key));
        append(&button, &el("span", "", &day.to_string()));
        if count > 0 {
            append(
                &button,
                &el(
                    "span",
                    "day-count",
                    if count > 99 {
                        "99+".into()
                    } else {
                        count.to_string()
                    }
                    .as_str(),
                ),
            );
        }
        let label = format!("{}: {count} {}", day_long(&key), records(count));
        set_attr(&button, "title", &label);
        set_attr(&button, "aria-label", &label);
        // Calendar cards are rebuilt, so the grid handles their clicks.
        append(&grid, &button);
    }
    let total = uint(&calendar, "total");
    let summary = if total == 0 {
        "Нет записей".into()
    } else {
        format!(
            "{} {} за месяц",
            number(total),
            plural(total, ["копия", "копии", "копий"])
        )
    };
    set_text("calendarSummary", &summary);
}

fn calendar_click(event: Event) {
    if let Some(button) = element_of(&event).and_then(|el| el.closest("[data-date]").ok().flatten())
    {
        select_day(Some(attr(&button, "data-date")));
    }
}
