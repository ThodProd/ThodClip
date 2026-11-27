use js_sys::{Date, Function, Promise, Reflect};
use serde_json::Value;
use std::{cell::LazyCell, future::Future};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Document, Element, Event, EventTarget, HtmlElement, HtmlInputElement, Window};

pub type BrowserResult<T> = Result<T, String>;

thread_local! {
    static ICONS: LazyCell<Value> = LazyCell::new(|| serde_json::from_str(include_str!("icons.json")).unwrap());
}

pub fn window() -> Window {
    web_sys::window().expect("browser window")
}
pub fn document() -> Document {
    window().document().expect("document")
}
pub fn id(name: &str) -> Element {
    document()
        .get_element_by_id(name)
        .unwrap_or_else(|| panic!("Missing element {name}"))
}
pub fn input(name: &str) -> HtmlInputElement {
    id(name).unchecked_into()
}
pub fn select(selector: &str) -> Vec<Element> {
    let nodes = document()
        .query_selector_all(selector)
        .expect("valid selector");
    (0..nodes.length())
        .filter_map(|index| nodes.item(index)?.dyn_into().ok())
        .collect()
}
pub fn el(tag: &str, class: &str, text: &str) -> Element {
    let element = document().create_element(tag).expect("valid tag");
    if !class.is_empty() {
        element.set_class_name(class);
    }
    if !text.is_empty() {
        element.set_text_content(Some(text));
    }
    element
}
pub fn append(parent: &Element, child: &Element) {
    parent.append_child(child).expect("append child");
}
pub fn set_attr(element: &Element, name: &str, value: &str) {
    element.set_attribute(name, value).expect("valid attribute");
}
pub fn attr(element: &Element, name: &str) -> String {
    element.get_attribute(name).unwrap_or_default()
}
pub fn set_text(name: &str, value: &str) {
    id(name).set_text_content(Some(value));
}
pub fn class(element: &Element, name: &str, on: bool) {
    element
        .class_list()
        .toggle_with_force(name, on)
        .expect("valid class");
}
pub fn hide(name: &str, value: bool) {
    id(name).unchecked_into::<HtmlElement>().set_hidden(value);
}
pub fn is_hidden(name: &str) -> bool {
    id(name).unchecked_into::<HtmlElement>().hidden()
}
pub fn focus(name: &str) {
    let _ = id(name).unchecked_into::<HtmlElement>().focus();
}
pub fn disabled(name: &str, value: bool) {
    disable(&id(name), value);
}
pub fn disable(element: &Element, value: bool) {
    if value {
        set_attr(element, "disabled", "");
    } else {
        let _ = element.remove_attribute("disabled");
    }
}
pub fn element_of(event: &Event) -> Option<Element> {
    event.target()?.dyn_into().ok()
}

pub fn icon(name: &str) -> Element {
    let span = el("span", "", "");
    set_attr(&span, "data-icon", name);
    icon_content(&span, name);
    span
}

fn icon_content(element: &Element, name: &str) {
    ICONS.with(|icons| {
        element.set_inner_html(
            icons[name]
                .as_str()
                .unwrap_or_else(|| icons["info"].as_str().unwrap()),
        )
    });
}

pub fn hydrate(root: &Element) {
    let nodes = root.query_selector_all("[data-icon]").unwrap();
    for index in 0..nodes.length() {
        if let Some(node) = nodes
            .item(index)
            .and_then(|node| node.dyn_into::<Element>().ok())
            && node.first_element_child().is_none()
        {
            icon_content(&node, &attr(&node, "data-icon"));
        }
    }
}

pub fn on(element: &EventTarget, name: &str, f: impl FnMut(Event) + 'static) {
    let f = Closure::wrap(Box::new(f) as Box<dyn FnMut(Event)>);
    element
        .add_event_listener_with_callback(name, f.as_ref().unchecked_ref())
        .unwrap();
    f.forget();
}

pub fn click(name: &str, mut f: impl FnMut() + 'static) {
    on(&id(name), "click", move |_| f());
}
pub fn timeout(milliseconds: i32, f: impl FnOnce() + 'static) -> i32 {
    let f = Closure::once_into_js(f);
    window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(f.unchecked_ref(), milliseconds)
        .unwrap()
}
pub fn interval(milliseconds: i32, f: impl FnMut() + 'static) -> i32 {
    let f = Closure::wrap(Box::new(f) as Box<dyn FnMut()>);
    let id = window()
        .set_interval_with_callback_and_timeout_and_arguments_0(
            f.as_ref().unchecked_ref(),
            milliseconds,
        )
        .unwrap();
    f.forget();
    id
}
pub fn frame(f: impl FnOnce() + 'static) {
    let f = Closure::once_into_js(f);
    window().request_animation_frame(f.unchecked_ref()).unwrap();
}
pub fn spawn(f: impl Future<Output = ()> + 'static) {
    spawn_local(f);
}

fn error_message(value: JsValue) -> String {
    Reflect::get(&value, &"message".into())
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| value.as_string())
        .unwrap_or_else(|| "Связь с ThodHUB недоступна.".into())
}

pub async fn bridge(method: &str, params: Value) -> BrowserResult<Value> {
    let hub = Reflect::get(&window(), &"thodHub".into()).map_err(error_message)?;
    let call: Function = Reflect::get(&hub, &"call".into())
        .map_err(error_message)?
        .dyn_into()
        .map_err(error_message)?;
    let params = params
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|error| error.to_string())?;
    let promise: Promise = call
        .call2(&hub, &method.into(), &params)
        .map_err(error_message)?
        .dyn_into()
        .map_err(error_message)?;
    let value = JsFuture::from(promise).await.map_err(error_message)?;
    serde_wasm_bindgen::from_value(value).map_err(|error| error.to_string())
}

pub async fn rpc(method: &str, params: Value) -> BrowserResult<Value> {
    bridge(&format!("worker.{method}"), params).await
}

pub fn overlay() {
    let visible = ["lightbox", "modalBackdrop", "settingsDrawer"]
        .iter()
        .any(|name| !is_hidden(name));
    class(&document().body().unwrap(), "has-overlay", visible);
}

pub fn toast(message: &str, symbol: &str, milliseconds: i32) {
    if message.is_empty() {
        return;
    }
    let item = el(
        "div",
        if symbol == "alert" {
            "toast is-error"
        } else {
            "toast"
        },
        "",
    );
    append(&item, &icon(symbol));
    append(&item, &el("span", "", message));
    let list = id("toasts");
    append(&list, &item);
    while list.child_element_count() > 4 {
        if let Some(first) = list.first_element_child() {
            first.remove();
        }
    }
    timeout(milliseconds, move || {
        if !item.is_connected() {
            return;
        }
        class(&item, "is-leaving", true);
        timeout(220, move || item.remove());
    });
}

pub fn error_toast(prefix: &str, error: &str) {
    toast(
        &format!(
            "{prefix}{}",
            error.strip_prefix("Error:").unwrap_or(error).trim_start()
        ),
        "alert",
        6500,
    );
}

pub fn or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() { fallback } else { value }
}
pub fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}
pub fn uint(value: &Value, key: &str) -> u64 {
    value[key].as_u64().unwrap_or_default()
}
pub fn flag(value: &Value, key: &str) -> bool {
    value[key].as_bool().unwrap_or_default()
}
pub fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().map_or(&[], Vec::as_slice)
}
pub fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|name| !name.is_empty())
        .unwrap_or_default()
}
pub fn number(n: u64) -> String {
    let value = n.to_string();
    let mut grouped = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index > 0 && (value.len() - index).is_multiple_of(3) {
            grouped.push('\u{a0}');
        }
        grouped.push(ch);
    }
    grouped
}
pub fn bytes(n: u64) -> String {
    if n < 1024 {
        return format!("{n} Б");
    }
    let mut size = n as f64 / 1024.0;
    let units = ["КБ", "МБ", "ГБ", "ТБ"];
    let mut index = 0;
    while size >= 1024.0 && index < units.len() - 1 {
        size /= 1024.0;
        index += 1;
    }
    let value = if size >= 10.0 {
        format!("{}", size.round())
    } else {
        format!("{size:.1}").replace('.', ",")
    };
    format!("{value} {}", units[index])
}

const MONTHS: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];
pub const MONTHS_NOM: [&str; 12] = [
    "Январь",
    "Февраль",
    "Март",
    "Апрель",
    "Май",
    "Июнь",
    "Июль",
    "Август",
    "Сентябрь",
    "Октябрь",
    "Ноябрь",
    "Декабрь",
];
pub fn plural(n: u64, forms: [&str; 3]) -> &str {
    if (11..=19).contains(&(n % 100)) {
        forms[2]
    } else {
        match n % 10 {
            1 => forms[0],
            2..=4 => forms[1],
            _ => forms[2],
        }
    }
}
pub fn records(n: u64) -> &'static str {
    plural(n, ["запись", "записи", "записей"])
}
pub fn current_month() -> (u32, u32) {
    let today = Date::new_0();
    (today.get_full_year(), today.get_month() + 1)
}
pub fn shift_month((year, month): (u32, u32), delta: i32) -> (u32, u32) {
    let date = Date::new_with_year_month_day(year, month as i32 - 1 + delta, 1);
    (date.get_full_year(), date.get_month() + 1)
}
pub fn month_from_key(key: &str) -> (u32, u32) {
    let mut parts = key.split('-');
    let year = parts.next().and_then(|n| n.parse().ok());
    let month = parts
        .next()
        .and_then(|n| n.parse().ok())
        .filter(|n| (1..=12).contains(n));
    year.zip(month).unwrap_or_else(current_month)
}
pub fn local_key(date: &Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date()
    )
}
pub fn local_date(iso: &str) -> String {
    local_key(&Date::new(&iso.into()))
}
pub fn day_long(key: &str) -> String {
    let (year, month) = month_from_key(key);
    let day = key
        .rsplit('-')
        .next()
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(1);
    format!("{day} {} {year}", MONTHS[month as usize - 1])
}
pub fn day_title(key: &str) -> String {
    if key == local_key(&Date::new_0()) {
        "Сегодня".into()
    } else if key == local_key(&Date::new(&JsValue::from_f64(Date::now() - 86_400_000.0))) {
        "Вчера".into()
    } else {
        day_long(key)
    }
}
pub fn short_date(iso: &str) -> String {
    let date = Date::new(&iso.into());
    if date.get_time().is_nan() {
        "—".into()
    } else {
        format!("{} {}", date.get_date(), MONTHS[date.get_month() as usize])
    }
}
pub fn date_time(iso: &str) -> String {
    let date = Date::new(&iso.into());
    if date.get_time().is_nan() {
        return "—".into();
    }
    format!("{:02}:{:02}", date.get_hours(), date.get_minutes())
}

use serde::Serialize;
