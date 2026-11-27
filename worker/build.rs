fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=../assets/thodclip.ico");
    let macros: &[&str] = if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bin=thodclip-worker=/MANIFEST:NO");
        &["THODCLIP_MANIFEST"]
    } else {
        // On MinGW, rustc supplies the default Windows application manifest.
        &[]
    };
    embed_resource::compile_for("app.rc", ["thodclip-worker"], macros)
        .manifest_required()
        .expect("embed Windows icon and manifest");
}
