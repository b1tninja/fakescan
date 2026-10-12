//! Windows: embed the Fs app icon into `fakescan.exe`.

fn main() {
    let icon = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/assets/fakescan.ico"
    );
    println!("cargo:rerun-if-changed={icon}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon(icon);
    res.set("ProductName", "fakescan")
        .set("FileDescription", "Fake WSD/WIA scanner for testing")
        .set(
            "LegalCopyright",
            "Copyright (c) the fakescan contributors. MIT OR Apache-2.0.",
        )
        .set("OriginalFilename", "fakescan.exe")
        .set("InternalName", "fakescan");
    if let Err(e) = res.compile() {
        println!("cargo:warning=fakescan.exe built without icon/resources: {e}");
    }
}
