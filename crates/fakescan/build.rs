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
    // The tray (native-windows-gui's subclass helpers) imports comctl32 v6 entry points by name;
    // without this manifest the exe fails to start (STATUS_ENTRYPOINT_NOT_FOUND).
    if std::env::var_os("CARGO_FEATURE_TRAY").is_some() {
        res.set_manifest(COMCTL_V6_MANIFEST);
    }
    if let Err(e) = res.compile() {
        println!("cargo:warning=fakescan.exe built without icon/resources: {e}");
    }
}

const COMCTL_V6_MANIFEST: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>"#;
