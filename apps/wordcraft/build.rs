//! Windows only: embed the app icon and version info (VERSIONINFO) into `quill.exe`, so it
//! shows in Explorer, the taskbar, the Start menu and Alt-Tab.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `WORDCRAFT_REQUIRE_WINRES=1` turns it
//! into an error (for release builds).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/quill.ico");
    println!("cargo:rerun-if-env-changed=WORDCRAFT_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/quill.ico")
        .set("ProductName", "Quill")
        .set("FileDescription", "Quill word processor")
        .set("CompanyName", "Quill contributors")
        .set("LegalCopyright", "Copyright (c) the Quill authors. MIT OR Apache-2.0.")
        .set("OriginalFilename", "quill.exe")
        .set("InternalName", "quill");
    if let Err(e) = res.compile() {
        if std::env::var_os("WORDCRAFT_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=quill.exe built without icon/version resources: {e}");
    }
}
