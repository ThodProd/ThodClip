# Third-party notices

ThodClip uses the Rust libraries pinned in [Cargo.lock](Cargo.lock). The principal runtime components include:

- [rusqlite](https://github.com/rusqlite/rusqlite) (MIT) and [SQLite](https://sqlite.org/copyright.html) (public domain).
- [image](https://github.com/image-rs/image), [notify](https://github.com/notify-rs/notify), [Serde](https://github.com/serde-rs/serde), [wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen) and [windows-sys](https://github.com/microsoft/windows-rs) (MIT or Apache 2.0, as applicable).
- [ICU4X](https://github.com/unicode-org/icu4x) and its locale data (Unicode License 3.0).

ThodClip uses Windows GDI+ and the Windows clipboard APIs. These are supplied by the operating system.

The ThodHUB postMessage bridge in `ui/thodhub.js` is copyright ThodProd and ships unchanged.
